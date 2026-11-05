//! The frame of a 0.26g world body, as a game reads it, held to the game's
//! fixed buffers (`docs/notes/world_frame_check.md`). One parse, shared by
//! two builds:
//! - the match server, before an uploaded world can go to another game
//!   (`match-server`, `game.rs::uploaded`);
//! - the game itself, in the exe's arena (patch 87,
//!   `client-patches/rust/patches/src/world_check.rs`), before its own
//!   reader takes a world it was sent (0xE8).
//!
//! So the crate is `no_std` with no `alloc` and no `unsafe`, and no read can
//! panic: a panic in the game is the crash the check is there to stop.
//!
//! The body (`game_state_history_item::serialize` 0x324BE0):
//!
//! ```text
//! u32 active_clients_mask
//! per set bit, ascending: u16 len, len bytes of state, u8 animated,
//!                         if animated: an animation bit stream
//! six times: u16 len + section (bullets, statistics, game objects,
//!                               physics, rules, effects)
//! ```
//!
//! A game reads it with no check of its own (0x324D80), so [`Frame::parse`]
//! takes only a body that:
//! - walks from the front to exactly its end: a length past the body reads
//!   past the game's buffer;
//! - has no mask bit at or past the match's [`Places`]: bit k goes into
//!   slot k with no bound, and the history item has 20;
//! - has no section over its buffer ([`Section::cap`]) and no player state
//!   over [`PLAYER_STATE_CAP`]: a longer one reaches
//!   `mutable_buffer::reallocate` 0x7AB70, which has no allocator (the fatal
//!   "mutable buffer overflow", then a call through null).
//!
//! Each is a length or a bit against a hard limit of the game's, so a world
//! a game made can never be refused: it serializes into the same buffers.

#![no_std]
#![forbid(unsafe_code)]
// No panic is reachable in the game's build: every index and every sum is
// checked.
#![cfg_attr(
    not(test),
    deny(
        clippy::indexing_slicing,
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::arithmetic_side_effects
    )
)]

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod tests;

use core::fmt;

/// The player slots of a game's history item (constructor 0x324510): a
/// match has at most this many places.
pub const MAX_PLAYERS: u8 = 20;

/// The buffer of a player's state (`player_history_item` 0x3247D0, read by
/// 0x33FD80). Real states have at most 0x3A8 bytes.
pub const PLAYER_STATE_CAP: usize = 0x1000;

/// A section of the body, in body order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Bullets,
    Statistics,
    Objects,
    Physics,
    Rules,
    Effects,
}

impl Section {
    /// The six, in the order of the body.
    pub const ALL: [Section; 6] = [
        Section::Bullets,
        Section::Statistics,
        Section::Objects,
        Section::Physics,
        Section::Rules,
        Section::Effects,
    ];

    /// The bytes after its u16 length that its buffer in a game's history
    /// item holds (0x324523..0x324680, 0x3246D0). Real worlds have at most
    /// 0x230, 0x45C, 0x3EC, 0, 0x76B and 0x57.
    pub const fn cap(self) -> usize {
        match self {
            Section::Bullets => 0x2400,
            Section::Statistics => 0xC00,
            Section::Objects => 0x2000,
            Section::Physics => 0x4000,
            Section::Rules => 0x800,
            Section::Effects => 0xC00,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Section::Bullets => "bullets",
            Section::Statistics => "statistics",
            Section::Objects => "game objects",
            Section::Physics => "physics",
            Section::Rules => "rules",
            Section::Effects => "effects",
        }
    }
}

/// The places a world may fill: the match's roster, at most
/// [`MAX_PLAYERS`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Places(u8);

impl Places {
    /// Every slot a game has: what a game checks against, which does not
    /// know the roster of the late join's growing matches.
    pub const ALL: Places = Places(MAX_PLAYERS);

    /// A roster of `roster` places; past [`MAX_PLAYERS`], all of them.
    pub fn of(roster: usize) -> Places {
        Places(u8::try_from(roster).map_or(MAX_PLAYERS, |roster| roster.min(MAX_PLAYERS)))
    }

    pub const fn count(self) -> u8 {
        self.0
    }
}

/// A player slot a world fills: a bit of its mask, below its [`Places`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slot(u8);

impl Slot {
    pub const fn index(self) -> u8 {
        self.0
    }
}

/// The active-clients mask of a world, every bit a [`Slot`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slots(u32);

impl Slots {
    /// `mask` read against `places`.
    fn parse(mask: u32, places: Places) -> Result<Slots, Refusal> {
        let past = mask.checked_shr(u32::from(places.count())).unwrap_or(0);
        if past == 0 {
            return Ok(Slots(mask));
        }
        // The lowest bit at or past the places.
        let bit = places
            .count()
            .saturating_add(past.trailing_zeros() as u8);
        Err(Refusal::SlotPastPlaces { bit, places })
    }

    pub const fn mask(self) -> u32 {
        self.0
    }

    /// The slots, ascending (the order of the player blocks).
    pub fn iter(self) -> impl Iterator<Item = Slot> {
        (0..MAX_PLAYERS)
            .filter(move |&bit| self.0.checked_shr(u32::from(bit)).unwrap_or(0) & 1 != 0)
            .map(Slot)
    }
}

/// A section of a body that fits its buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SectionBytes<'a> {
    /// Where its bytes start in the body (after the u16 length).
    pub at: usize,
    pub bytes: &'a [u8],
}

/// A body a game can load: its slots, their player states and its six
/// sections.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frame<'a> {
    slots: Slots,
    /// Each slot's state (`base_player::serialize`'s bytes), by slot index;
    /// empty for a slot the mask does not have.
    states: [&'a [u8]; MAX_PLAYERS as usize],
    sections: [SectionBytes<'a>; 6],
}

impl<'a> Frame<'a> {
    /// `body` (from the mask on) read from the front.
    pub fn parse(body: &'a [u8], places: Places) -> Result<Frame<'a>, Refusal> {
        let mut reader = Reader::new(body);
        let mask = reader.u32().ok_or(Refusal::Short {
            part: Part::Mask,
            at: 0,
        })?;
        let slots = Slots::parse(mask, places)?;
        let mut states: [&'a [u8]; MAX_PLAYERS as usize] = [&[]; MAX_PLAYERS as usize];
        for slot in slots.iter() {
            let short = |at| Refusal::Short {
                part: Part::Player(slot),
                at,
            };
            let len = usize::from(reader.u16().ok_or(short(reader.at))?);
            if len > PLAYER_STATE_CAP {
                return Err(Refusal::StateOverCap {
                    slot,
                    at: reader.at,
                    len,
                });
            }
            let state = reader.take(len).ok_or(short(reader.at))?;
            if let Some(room) = states.get_mut(usize::from(slot.0)) {
                *room = state;
            }
            if reader.u8().ok_or(short(reader.at))? != 0 {
                let len = animation_len(reader.rest()).ok_or(short(reader.at))?;
                reader.take(len).ok_or(short(reader.at))?;
            }
        }
        let mut sections = [SectionBytes { at: 0, bytes: &[] }; 6];
        for (section, place) in Section::ALL.into_iter().zip(&mut sections) {
            let short = |at| Refusal::Short {
                part: Part::Section(section),
                at,
            };
            let len = usize::from(reader.u16().ok_or(short(reader.at))?);
            let at = reader.at;
            if len > section.cap() {
                return Err(Refusal::SectionOverCap { section, at, len });
            }
            let bytes = reader.take(len).ok_or(short(at))?;
            *place = SectionBytes { at, bytes };
        }
        match reader.rest().len() {
            0 => Ok(Frame { slots, states, sections }),
            len => Err(Refusal::Trailing { at: reader.at, len }),
        }
    }

    /// A world as the game's reader has it at the load (0x2D0B70): its time
    /// (u32), then the body. Offsets are the body's.
    pub fn parse_state(state: &'a [u8], places: Places) -> Result<Frame<'a>, Refusal> {
        let (_time, body) = state.split_first_chunk::<4>().ok_or(Refusal::NoTime)?;
        Frame::parse(body, places)
    }

    pub fn slots(&self) -> Slots {
        self.slots
    }

    /// Each slot's player state, ascending (the order of the player
    /// blocks): what `base_player::serialize` wrote.
    pub fn states(&self) -> impl Iterator<Item = (Slot, &'a [u8])> + '_ {
        self.slots
            .iter()
            .filter_map(|slot| Some((slot, *self.states.get(usize::from(slot.0))?)))
    }

    pub fn section(&self, section: Section) -> SectionBytes<'a> {
        let [bullets, statistics, objects, physics, rules, effects] = self.sections;
        match section {
            Section::Bullets => bullets,
            Section::Statistics => statistics,
            Section::Objects => objects,
            Section::Physics => physics,
            Section::Rules => rules,
            Section::Effects => effects,
        }
    }
}

/// Where in a body a refusal is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Part {
    /// The world's time, before the body (a game's state only).
    Time,
    Mask,
    Player(Slot),
    Section(Section),
    /// After the effects.
    End,
}

impl Part {
    /// Its words, without a player's slot.
    pub const fn name(self) -> &'static str {
        match self {
            Part::Time => "time",
            Part::Mask => "mask",
            Part::Player(_) => "player",
            Part::Section(section) => section.name(),
            Part::End => "end",
        }
    }
}

impl fmt::Display for Part {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Part::Player(slot) => write!(f, "player {}", slot.index()),
            part => f.write_str(part.name()),
        }
    }
}

/// Why a body is no world a game can load.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The state is shorter than the world's time.
    NoTime,
    /// The bytes end inside `part`, at `at`.
    Short { part: Part, at: usize },
    /// The mask has bit `bit`, at or past the places.
    SlotPastPlaces { bit: u8, places: Places },
    /// A player state of `len` bytes, over [`PLAYER_STATE_CAP`].
    StateOverCap { slot: Slot, at: usize, len: usize },
    /// A section of `len` bytes, over its [`Section::cap`].
    SectionOverCap {
        section: Section,
        at: usize,
        len: usize,
    },
    /// `len` bytes after the effects.
    Trailing { at: usize, len: usize },
}

impl Refusal {
    pub const fn part(self) -> Part {
        match self {
            Refusal::NoTime => Part::Time,
            Refusal::Short { part, .. } => part,
            Refusal::SlotPastPlaces { .. } => Part::Mask,
            Refusal::StateOverCap { slot, .. } => Part::Player(slot),
            Refusal::SectionOverCap { section, .. } => Part::Section(section),
            Refusal::Trailing { .. } => Part::End,
        }
    }

    /// Its offset in the body.
    pub const fn at(self) -> usize {
        match self {
            Refusal::NoTime | Refusal::SlotPastPlaces { .. } => 0,
            Refusal::Short { at, .. }
            | Refusal::StateOverCap { at, .. }
            | Refusal::SectionOverCap { at, .. }
            | Refusal::Trailing { at, .. } => at,
        }
    }

    /// Its reason, in words without numbers (the game's log line).
    pub const fn reason(self) -> &'static str {
        match self {
            Refusal::NoTime => "shorter than a time",
            Refusal::Short { .. } => "the bytes end inside it",
            Refusal::SlotPastPlaces { .. } => "a player past the places",
            Refusal::StateOverCap { .. } => "a state over its buffer",
            Refusal::SectionOverCap { .. } => "over its buffer",
            Refusal::Trailing { .. } => "bytes after the effects",
        }
    }
}

/// `<part> at <offset>: <reason>`, the server's warning text.
impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at {:#x}: ", self.part(), self.at())?;
        match *self {
            Refusal::SlotPastPlaces { bit, places } => write!(
                f,
                "a player in slot {bit}, past the {} places",
                places.count()
            ),
            Refusal::StateOverCap { len, .. } => {
                write!(f, "a state of {len:#x} bytes, over {PLAYER_STATE_CAP:#x}")
            }
            Refusal::SectionOverCap { section, len, .. } => {
                write!(f, "{len:#x} bytes, over {:#x}", section.cap())
            }
            Refusal::Trailing { len, .. } => write!(f, "{len} bytes after the effects"),
            refusal => f.write_str(refusal.reason()),
        }
    }
}

/// A cursor over a body; every read is `None` past its end.
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Reader { bytes, at: 0 }
    }

    fn rest(&self) -> &'a [u8] {
        self.bytes.get(self.at..).unwrap_or_default()
    }

    fn take(&mut self, len: usize) -> Option<&'a [u8]> {
        let (taken, _) = self.rest().split_at_checked(len)?;
        self.at = self.at.checked_add(len)?;
        Some(taken)
    }

    fn array<const N: usize>(&mut self) -> Option<[u8; N]> {
        let (taken, _) = self.rest().split_first_chunk::<N>()?;
        self.at = self.at.checked_add(N)?;
        Some(*taken)
    }

    fn u8(&mut self) -> Option<u8> {
        self.array().map(u8::from_le_bytes)
    }

    fn u16(&mut self) -> Option<u16> {
        self.array().map(u16::from_le_bytes)
    }

    fn u32(&mut self) -> Option<u32> {
        self.array().map(u32::from_le_bytes)
    }
}

/// Reads bits most significant first, as the animation encoder writes them
/// (0x3A8980).
struct Bits<'a> {
    bytes: &'a [u8],
    /// In bits from the start.
    at: usize,
}

impl Bits<'_> {
    fn read(&mut self, bits: usize) -> Option<u32> {
        let end = self.at.checked_add(bits)?;
        if bits > 32 || end > self.bytes.len().checked_mul(8)? {
            return None;
        }
        let mut value = 0u64;
        while self.at < end {
            let byte = *self.bytes.get(self.at / 8)?;
            let bit = byte.checked_shr(7 ^ (self.at % 8) as u32)? & 1;
            value = value.checked_shl(1)? | u64::from(bit);
            self.at = self.at.checked_add(1)?;
        }
        Some(value as u32)
    }

    fn skip(&mut self, bits: usize) -> Option<()> {
        self.at = self.at.checked_add(bits)?;
        (self.at <= self.bytes.len().checked_mul(8)?).then_some(())
    }
}

/// The bits an index below `count` takes (0x3A961E, 0x396120).
fn width(count: u32) -> usize {
    (u32::BITS.saturating_sub(count.leading_zeros())).max(1) as usize
}

/// How many bytes the animation state at the start of `bytes` takes
/// (`decode_data` 0x3A9900): 16 bits `T`, the tree part of `T` bits, the
/// times table (0x3A9580), the floats table (0x3A9720), padded to a byte.
/// `None`: the bytes end first.
pub fn animation_len(bytes: &[u8]) -> Option<usize> {
    let mut bits = Bits { bytes, at: 0 };
    let tree = bits.read(16)? as usize;
    bits.skip(tree)?;
    // Times: a count, then either each time or a table of the different
    // ones and an index for each.
    let count = bits.read(10)?;
    if count != 0 {
        let indexed = bits.read(1)? != 0;
        bits.skip(32)?;
        if count > 1 {
            let each = bits.read(5)? as usize;
            if indexed {
                let unique = bits.read(width(count))?;
                bits.skip((unique.saturating_sub(1) as usize).checked_mul(each)?)?;
                bits.skip((count as usize).checked_mul(width(unique))?)?;
            } else {
                bits.skip((count as usize).checked_mul(each)?)?;
            }
        }
    }
    // Floats: the same two forms, 32 bits each.
    let indexed = bits.read(1)? != 0;
    let count = bits.read(10)?;
    if indexed {
        let unique = bits.read(width(count))?;
        bits.skip((unique as usize).checked_mul(32)?)?;
        bits.skip((count as usize).checked_mul(width(unique))?)?;
    } else {
        bits.skip((count as usize).checked_mul(32)?)?;
    }
    Some(bits.at.div_ceil(8))
}
