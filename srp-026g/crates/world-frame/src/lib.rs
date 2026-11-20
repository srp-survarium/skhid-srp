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
//! - is at most [`MAX_BODY`] bytes: the game hashes every world it holds
//!   (`game_state_history_item::hash` 0x325150) in a buffer of that size;
//! - walks from the front to exactly its end: a length past the body reads
//!   past the game's buffer;
//! - has no mask bit at or past the match's [`Places`]: bit k goes into
//!   slot k with no bound, and the history item has 20;
//! - has no player state over [`PLAYER_STATE_CAP`]: a longer one reaches
//!   `mutable_buffer::reallocate` 0x7AB70, which has no allocator for it
//!   (the fatal "mutable buffer overflow", then a call through null).
//!
//! The six sections have no buffer of their own to fit: patch 98 gives each
//! the game's multithreaded allocator, so they grow (a section is under
//! [`MAX_BODY`] with the body).
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

/// The bytes of a body: `game_state_history_item::hash` (0x325150)
/// serializes the world into a stack buffer of this size, with no allocator
/// (0x8000 in the original exe; patch 98's 0xF000, which the snapshot
/// upload's u16 total holds). A longer world asserts there.
pub const MAX_BODY: usize = 0xF000;

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

/// A section of a body.
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
        if body.len() > MAX_BODY {
            return Err(Refusal::OverBuffer { len: body.len() });
        }
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
                let at = reader.at;
                let len = animation_len(reader.rest()).ok_or(short(at))?;
                animation_validate(reader.rest())
                    .map_err(|fault| Refusal::Animation { slot, at, fault })?;
                reader.take(len).ok_or(short(at))?;
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
    /// The body as a whole.
    Body,
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
            Part::Body => "body",
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
    /// A body of `len` bytes, over [`MAX_BODY`].
    OverBuffer { len: usize },
    /// The bytes end inside `part`, at `at`.
    Short { part: Part, at: usize },
    /// The mask has bit `bit`, at or past the places.
    SlotPastPlaces { bit: u8, places: Places },
    /// A player state of `len` bytes, over [`PLAYER_STATE_CAP`].
    StateOverCap { slot: Slot, at: usize, len: usize },
    /// `len` bytes after the effects.
    Trailing { at: usize, len: usize },
    /// A player's animation state at `at` is malformed in a way a native
    /// client over-reads on (`animation_validate`).
    Animation { slot: Slot, at: usize, fault: AnimFault },
}

impl Refusal {
    pub const fn part(self) -> Part {
        match self {
            Refusal::NoTime => Part::Time,
            Refusal::OverBuffer { .. } => Part::Body,
            Refusal::Short { part, .. } => part,
            Refusal::SlotPastPlaces { .. } => Part::Mask,
            Refusal::StateOverCap { slot, .. } => Part::Player(slot),
            Refusal::Trailing { .. } => Part::End,
            Refusal::Animation { slot, .. } => Part::Player(slot),
        }
    }

    /// Its offset in the body.
    pub const fn at(self) -> usize {
        match self {
            Refusal::NoTime | Refusal::OverBuffer { .. } | Refusal::SlotPastPlaces { .. } => 0,
            Refusal::Short { at, .. }
            | Refusal::StateOverCap { at, .. }
            | Refusal::Trailing { at, .. }
            | Refusal::Animation { at, .. } => at,
        }
    }

    /// Its reason, in words without numbers (the game's log line).
    pub const fn reason(self) -> &'static str {
        match self {
            Refusal::NoTime => "shorter than a time",
            Refusal::OverBuffer { .. } => "over the game's world buffer",
            Refusal::Short { .. } => "the bytes end inside it",
            Refusal::SlotPastPlaces { .. } => "a player past the places",
            Refusal::StateOverCap { .. } => "a state over its buffer",
            Refusal::Trailing { .. } => "bytes after the effects",
            Refusal::Animation { .. } => "an animation a client over-reads",
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
            Refusal::OverBuffer { len } => write!(f, "{len:#x} bytes, over {MAX_BODY:#x}"),
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
        // A byte's run of bits at a time, most significant first: at most
        // five steps for 32 bits (one bit a step made the floats table's
        // checks the bulk of a world check's instructions).
        let mut value = 0u64;
        while self.at < end {
            let byte = *self.bytes.get(self.at / 8)?;
            let left = 8usize.checked_sub(self.at % 8)?;
            let take = left.min(end.checked_sub(self.at)?);
            let below = u32::try_from(left.checked_sub(take)?).ok()?;
            let width = u32::try_from(take).ok()?;
            let mask = u8::MAX.checked_shr(8u32.checked_sub(width)?)?;
            let run = byte.checked_shr(below)? & mask;
            value = value.checked_shl(width)? | u64::from(run);
            self.at = self.at.checked_add(take)?;
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

/// Why the animation state is refused: a value a native client reads as a
/// count or index without bounding it (`snapshot_rules.md` 7.2.1). The length
/// walk ([`animation_len`]) does not catch these; [`animation_validate`] does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimFault {
    /// The bytes end inside the state.
    Short,
    /// A times-table index is at or past the table's unique count (0x3A96F2).
    TimesIndex,
    /// A floats-table index is at or past the table's unique count (0x3A9895).
    FloatsIndex,
    /// A channel pick is at or past the channel count (0x3A8EF0).
    ChannelIndex,
    /// The expression trees exceed the node cap (0x3A90C0 recurses unbounded).
    Recursion,
    /// The structural tree does not end exactly at the `T`-bit mark.
    TreeLength,
    /// The tree pops more times or floats than the two tables hold.
    TableUnderrun,
    /// A floats-table value is infinity or NaN: a plain one, or an indexed
    /// table's entry. The native decoder feeds the floats to bone transforms
    /// that a ragdoll and a bounding box then read.
    FloatNotNumber,
}

/// A native client decodes far more of the animation state than the length
/// walk: the whole `T`-bit tree part (`anim_state_deserialize` 0x3A99C0). It
/// reads several values as a count or index against a sized buffer without a
/// bound, so a crafted stream over-reads or (the expression recursion) runs
/// the thread stack out. Decode the tree the way the client does, with a node
/// cap and the bounds of `snapshot_rules.md` 7.2.1, and refuse on any breach.
/// `Ok(())` means a native client decodes it within its buffers.
///
/// The expression forest is a binary tree the client reads pre-order; a
/// pending-node counter reproduces that read order exactly and caps the depth
/// without recursing, so validating cannot itself overflow the stack.
pub fn animation_validate(bytes: &[u8]) -> Result<(), AnimFault> {
    use AnimFault::Short;
    /// The whole tree is `T` <= 65535 bits and each expression node costs at
    /// least 2 bits, so a valid forest holds under 32768 nodes; more than this
    /// is malformed. The counter below is O(1) memory, so the cap only bounds
    /// work and gives a clear reason -- above 32768 it never refuses a real
    /// stream (none of the 177,672 animated blocks of the kept worlds is close).
    const MAX_EXPR_NODES: u32 = 1 << 16;

    let mut b = Bits { bytes, at: 0 };
    let tree = b.read(16).ok_or(Short)? as usize;
    let tree_end = 16usize.checked_add(tree).ok_or(Short)?;

    // The native decode pops one time before any node pass: an unconditional
    // "duration" read off the times stack (0x3A9A64, `mov eax,[edi+8];
    // mov eax,[eax-4]; add [edi+8],-4`). The length walk and the passes below
    // never model it, so seed the pop count with it; otherwise a stream whose
    // times table holds exactly the modelled pops lets the native read one
    // entry below the buffer.
    let mut pops_times: u64 = 1;
    let mut pops_floats: u64 = 0;

    // Tree header (0x3A99C0): node count, matrix count bit, channel mask.
    let nodes = b.read(6).ok_or(Short)?;
    b.read(1).ok_or(Short)?; // the matrix count (1 or 2); not needed here
    let channels = b.read(4).ok_or(Short)?.count_ones();

    // Base tracks (anim_read_node_base_tracks 0x3A8C10): two passes over the
    // nodes, not one interleaved pass. First pass: four bits and two popped
    // floats a node.
    for _ in 0..nodes {
        for _ in 0..4 {
            b.read(1).ok_or(Short)?;
        }
        pops_floats = pops_floats.saturating_add(2);
    }
    // Second pass: a time track (0x3A8AF0) and a flag track (0x3A8BA0) a node.
    for _ in 0..nodes {
        // Time track: a 6-bit count, then a record when it is set.
        if b.read(6).ok_or(Short)? != 0 {
            b.read(1).ok_or(Short)?;
            b.read(8).ok_or(Short)?;
            b.read(4).ok_or(Short)?;
            pops_times = pops_times.saturating_add(1);
            pops_floats = pops_floats.saturating_add(1);
        }
        // Flag track: two bits, a time when either is set.
        let f0 = b.read(1).ok_or(Short)?;
        let f1 = b.read(1).ok_or(Short)?;
        if f0 != 0 || f1 != 0 {
            pops_times = pops_times.saturating_add(1);
        }
    }

    // Matrix init (0x3A8D60): the first matrix's 12 affine floats, no bits.
    pops_floats = pops_floats.saturating_add(12);

    // Node records (anim_read_node_record 0x3A8F40); sum the expression roots.
    let mut roots: u64 = 0;
    for _ in 0..nodes {
        b.read(4).ok_or(Short)?;
        b.read(1).ok_or(Short)?;
        // The M entry count is this bit plus one, so 0..=bit is M entries.
        let m = b.read(1).ok_or(Short)?;
        for _ in 0..=m {
            b.read(10).ok_or(Short)?;
            b.read(1).ok_or(Short)?;
        }
        b.read(1).ok_or(Short)?;
        channel_pick(&mut b, channels)?;
        channel_pick(&mut b, channels)?;
        for width in [1usize, 5, 2, 1, 3, 3, 2, 1, 1] {
            b.read(width).ok_or(Short)?;
        }
        roots = roots.saturating_add(u64::from(b.read(2).ok_or(Short)?));
    }

    // Expression forest (anim_build_node 0x3A9250 -> anim_read_expression
    // 0x3A90C0), pre-order with a node cap in place of the native recursion.
    let mut pending = roots;
    let mut budget = MAX_EXPR_NODES;
    while pending > 0 {
        budget = budget.checked_sub(1).ok_or(AnimFault::Recursion)?;
        pending = pending.saturating_sub(1);
        let op = b.read(2).ok_or(Short)?;
        channel_pick(&mut b, channels)?;
        match op {
            // op 0 (0x3A90D8) pops two floats from the floats buffer AND one
            // time from the times buffer (0x3A9116, `mov eax,[edi+8];
            // mov eax,[eax-4]; add [edi+8],-4`). Counting only the floats let a
            // forest of op-0 nodes walk the times pointer past its buffer.
            0 => {
                pops_floats = pops_floats.saturating_add(2);
                pops_times = pops_times.saturating_add(1);
            }
            1 => pops_floats = pops_floats.saturating_add(1),
            // Ops 2 and 3 take two child expressions.
            _ => {
                pops_times = pops_times.saturating_add(1);
                pending = pending.saturating_add(2);
            }
        }
    }

    // The client decodes the tree structurally and ignores `T`; a stream whose
    // structure is not exactly `T` bits desyncs its cursor from the framing.
    if b.at != tree_end {
        return Err(AnimFault::TreeLength);
    }

    let times = table_count(&mut b, true)?;
    let floats = table_count(&mut b, false)?;
    if pops_times > times || pops_floats > floats {
        return Err(AnimFault::TableUnderrun);
    }
    Ok(())
}

/// A channel pick (`anim_pick_channel_value` 0x3A8EF0): the index into the
/// `channels`-entry array, in as many bits as the native code reads. The
/// native code does not bound it, so refuse an index that is not there.
fn channel_pick(b: &mut Bits<'_>, channels: u32) -> Result<(), AnimFault> {
    let index = match channels {
        0 => return Err(AnimFault::ChannelIndex),
        1 => 0,
        2 => b.read(1).ok_or(AnimFault::Short)?,
        _ => b.read(2).ok_or(AnimFault::Short)?,
    };
    if index >= channels {
        return Err(AnimFault::ChannelIndex);
    }
    Ok(())
}

/// One 32-bit float of the floats table: refuse infinity and NaN with the
/// same bit test as `srp-validate`'s `finite` (the exponent not all ones), so
/// a crafted stream cannot put a non-number into a bone transform.
fn finite_float(b: &mut Bits<'_>) -> Result<(), AnimFault> {
    if b.read(32).ok_or(AnimFault::Short)? & 0x7F80_0000 != 0x7F80_0000 {
        Ok(())
    } else {
        Err(AnimFault::FloatNotNumber)
    }
}

/// One table: `decode_times_in_ms_impl` 0x3A9580 when `times`, else
/// `decode_floats_impl` 0x3A9720. Read it as the length walk does but bound
/// each index to the indexes the game's encoder writes, and return how many
/// values it pushes (what the tree may pop). The two order their count and
/// mode bit differently.
///
/// The encoders (`save_times_in_ms` 0x396D40, `save_floats` 0x397040) sort
/// the values into a stack copy, keep the unique ones (`unique` 0x397810 for
/// the floats: `ucomiss` equality) and write for each value the position
/// `find` (0x397880, linear, the same equality) gives it, so every index is
/// below the unique count and every entry is some value's. Only a NaN is not
/// found (it equals nothing: its index would be the count, `table[unique]`
/// the dword past the reader's `_alloca_probe_16` table, 0x3A97D1), and a
/// game with exe patch 117 has no NaN to save (`snapshot_rules.md` 7.2.1): a
/// floats table holds numbers only and its indexes are below the count, as a
/// times table's are.
fn table_count(b: &mut Bits<'_>, times: bool) -> Result<u64, AnimFault> {
    use AnimFault::Short;
    if times {
        let count = b.read(10).ok_or(Short)?;
        if count != 0 {
            let indexed = b.read(1).ok_or(Short)? != 0;
            b.read(32).ok_or(Short)?;
            if count > 1 {
                let each = b.read(5).ok_or(Short)? as usize;
                if indexed {
                    let unique = b.read(width(count)).ok_or(Short)?;
                    b.skip((unique.saturating_sub(1) as usize).checked_mul(each).ok_or(Short)?)
                        .ok_or(Short)?;
                    for _ in 0..count {
                        if b.read(width(unique)).ok_or(Short)? >= unique {
                            return Err(AnimFault::TimesIndex);
                        }
                    }
                } else {
                    b.skip((count as usize).checked_mul(each).ok_or(Short)?).ok_or(Short)?;
                }
            }
        }
        Ok(u64::from(count))
    } else {
        let indexed = b.read(1).ok_or(Short)? != 0;
        let count = b.read(10).ok_or(Short)?;
        if indexed {
            let unique = b.read(width(count)).ok_or(Short)?;
            for _ in 0..unique {
                finite_float(b)?;
            }
            for _ in 0..count {
                if b.read(width(unique)).ok_or(Short)? >= unique {
                    return Err(AnimFault::FloatsIndex);
                }
            }
        } else {
            for _ in 0..count {
                finite_float(b)?;
            }
        }
        Ok(u64::from(count))
    }
}
