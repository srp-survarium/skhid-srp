//! The frame of a world body (match.md section 36, snapshot_rules.md
//! section 7):
//!
//! ```text
//! u32 active_clients_mask
//! per set bit, ascending: u16 L, L bytes of state, u8 animated,
//!                         if animated: an animation bit stream
//! six times: u16 len + section (bullets, statistics, game objects,
//!                               physics, rules, effects)
//! ```

use crate::reader::{Reader, Short};
use crate::{MAX_PLAYERS, Part, Reason, Refusal};

/// The buffer each section of a world is read into, in a game's history
/// item (constructor 0x324510, 0x324523..0x324680, 0x3246D0): bullets,
/// statistics, objects, physics, rules, effects. The cap is of the bytes
/// after the u16 length. A longer section goes to
/// `mutable_buffer::reallocate` 0x7AB70, which has no allocator: the fatal
/// "mutable buffer overflow", then a call through null. PROVEN (match.md
/// section 36). Real worlds have at most 0x230, 0x45C, 0x3EC, 0, 0x76B
/// and 0x57 (kept worlds of real games, 2026-10-12..21).
pub const SECTION_CAPS: [usize; 6] = [0x2400, 0xC00, 0x2000, 0x4000, 0x800, 0xC00];
/// The buffer of a player's state (`player_history_item` 0x3247D0, read
/// by 0x33FD80), as [`SECTION_CAPS`]. Real states have at most 0x3A8.
pub const PLAYER_STATE_CAP: usize = 0x1000;

/// One player's block in a body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Block<'a> {
    /// The roster place: the bit of the mask.
    pub place: u8,
    /// Where the state starts in the body.
    pub at: usize,
    /// `player_serialized_state`, without its length.
    pub state: &'a [u8],
}

/// A section of a body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Section<'a> {
    /// Where its bytes start in the body (after the u16 length).
    pub at: usize,
    pub bytes: &'a [u8],
}

/// A body that walks: its mask and its six sections in body order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frame<'a> {
    pub active_clients_mask: u32,
    pub sections: [Section<'a>; 6],
}

impl<'a> Frame<'a> {
    pub fn bullets(&self) -> Section<'a> {
        let [bullets, ..] = self.sections;
        bullets
    }

    pub fn effects(&self) -> Section<'a> {
        let [.., effects] = self.sections;
        effects
    }

    /// Whether the mask has a player block at `place`.
    pub fn has_block(&self, place: u8) -> bool {
        self.active_clients_mask
            .checked_shr(u32::from(place))
            .unwrap_or(0)
            & 1
            != 0
    }

    pub fn statistics(&self) -> Section<'a> {
        let [_, statistics, ..] = self.sections;
        statistics
    }

    pub fn objects(&self) -> Section<'a> {
        let [_, _, objects, ..] = self.sections;
        objects
    }

    pub fn physics(&self) -> Section<'a> {
        let [_, _, _, physics, ..] = self.sections;
        physics
    }

    pub fn rules(&self) -> Section<'a> {
        let [_, _, _, _, rules, _] = self.sections;
        rules
    }
}

/// The player blocks of a body, walked from the front.
#[derive(Clone, Copy, Debug)]
pub struct Blocks<'a> {
    reader: Reader<'a>,
    mask: u32,
    /// The next bit of the mask to look at.
    place: u8,
}

impl<'a> Blocks<'a> {
    /// The walk over `body`'s blocks. As an iterator it ends at the first
    /// block that does not read: for a body that [`frame`] took, that is
    /// after the last.
    pub fn of(body: &'a [u8]) -> Self {
        let mut reader = Reader::new(body, 0);
        let mask = reader.u32().unwrap_or(0);
        Self {
            reader,
            mask,
            place: 0,
        }
    }

    /// The next block, `Ok(None)` after the last.
    pub fn next_block(&mut self) -> Result<Option<Block<'a>>, Short> {
        while usize::from(self.place) < 32 {
            let place = self.place;
            self.place = place.saturating_add(1);
            if self.mask.checked_shr(u32::from(place)).unwrap_or(0) & 1 == 0 {
                continue;
            }
            let len = usize::from(self.reader.u16()?);
            let at = self.reader.at();
            let state = self.reader.take(len)?;
            if self.reader.u8()? != 0 {
                let start = self.reader.at();
                let len = animation_len(self.reader.rest()).ok_or(Short {
                    at: start,
                    wanted: 2,
                })?;
                self.reader.skip(len)?;
            }
            return Ok(Some(Block { place, at, state }));
        }
        Ok(None)
    }

    /// Walks every block: where the first section's length is.
    pub fn end(mut self) -> Result<usize, Short> {
        while self.next_block()?.is_some() {}
        Ok(self.reader.at())
    }
}

impl<'a> Iterator for Blocks<'a> {
    type Item = Block<'a>;

    fn next(&mut self) -> Option<Block<'a>> {
        self.next_block().ok().flatten()
    }
}

/// The frame (match.md section 36): it must walk from the front (the
/// reader has no end, 0x324D80), its mask name no slot past the roster
/// (bit k is slot k with no bound, 20 slots), and no section nor player
/// state be over its buffer (the fatal "mutable buffer overflow").
pub fn frame(body: &[u8], places: usize) -> Result<Frame<'_>, Refusal> {
    let refuse = |reason| Refusal::new(Part::Frame, 0, reason);
    // One walk: where the blocks end, and the first state over its buffer.
    let walk = || -> Result<(u32, usize, Option<Block<'_>>), Short> {
        let mask = Reader::new(body, 0).u32()?;
        let mut blocks = Blocks::of(body);
        let mut over = None;
        while let Some(block) = blocks.next_block()? {
            if over.is_none() && block.state.len() > PLAYER_STATE_CAP {
                over = Some(block);
            }
        }
        Ok((mask, blocks.reader.at(), over))
    };
    let Ok((mask, players_end, over)) = walk() else {
        return Err(refuse(Reason::NoWalk));
    };
    let Some(sections) = sections(body, players_end) else {
        return Err(refuse(Reason::NoSections));
    };
    let places = places.min(MAX_PLAYERS);
    if let Some(place) =
        (places..32).find(|bit| mask.checked_shr(*bit as u32).unwrap_or(0) & 1 != 0)
    {
        return Err(refuse(Reason::PlaceOutside { place, places }));
    }
    if let Some(block) = over {
        return Err(Refusal::new(
            Part::Player(block.place),
            block.at,
            Reason::StateOverCap {
                len: block.state.len(),
            },
        ));
    }
    for ((section, cap), part) in sections.iter().zip(SECTION_CAPS).zip(Part::SECTIONS) {
        if section.bytes.len() > cap {
            return Err(Refusal::new(
                part,
                section.at,
                Reason::SectionOverCap {
                    len: section.bytes.len(),
                    cap,
                },
            ));
        }
    }
    Ok(Frame {
        active_clients_mask: mask,
        sections,
    })
}

/// Six length-prefixed sections from `at` that end exactly at the body's
/// end.
fn sections(body: &[u8], at: usize) -> Option<[Section<'_>; 6]> {
    let mut reader = Reader::new(body.get(at..)?, at);
    let mut sections = [Section { at, bytes: &[] }; 6];
    for section in &mut sections {
        let len = usize::from(reader.u16().ok()?);
        *section = Section {
            at: reader.at(),
            bytes: reader.take(len).ok()?,
        };
    }
    reader.is_done().then_some(sections)
}

/// Reads bits most significant first, as the animation encoder writes
/// them (0x3A8980).
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
