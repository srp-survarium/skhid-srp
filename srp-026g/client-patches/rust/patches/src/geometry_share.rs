//! Patch 164: a vertex or index buffer with the same bytes as a live one in
//! the render's geometry pool takes that one's range of the pool instead of
//! a copy (`client_patch.md` section 3.88).
//!
//! A character's skin (profile_skin_visual_cook) is its own combined model:
//! skeleton_combined_model_cook makes every part's mesh again from the
//! part's file, so each skin holds its own copy of each part's vertices and
//! indices, and two skins that share a torso hold it twice. Faces make more
//! skins (two players in the same gear with other faces are two), and the
//! copies filled the 88 MiB vertex pool in a 10 v 10 (+21 MiB of vertices,
//! +4 MiB of indices for 30 more skins, of which the faces' own models are a
//! few MiB).
//!
//! A pooled buffer's bytes are never written after it is made (the pool
//! copies them in once, hw_buffer_pool_chunk::allocate_range 0x3D3D60; its
//! users only read the range), so buffers with the same bytes, stride and
//! pool can share one range. [`Table`] keeps each live range of at least
//! [`SMALLEST`] bytes with what is in it and how many buffers hold it:
//!
//! - hw_buffer_pool::allocate (`on_allocate`): bytes a range already holds
//!   get that range, the pool not asked;
//! - the buffer's constructor once it has a range (`on_pooled`): a new
//!   range is kept;
//! - hw_buffer_pool::deallocate (`on_deallocate`): a range another buffer
//!   still holds stays in its chunk.
//!
//! The game's threads make and give back buffers: the table is a [`Lock`].
//! Drawn the same: a range's bytes are the bytes each buffer was made with.

#![forbid(unsafe_code)]

use crate::vostok::memory::{released_range, Content, PoolAllocation, PoolRange, PooledBuffer};
use crate::vostok::{Context, Flow, GeometryShareOnAllocateExit, GeometryShareOnDeallocateExit, Lock, NoExit, Thread};

/// Ranges kept at most; past it a new range is not shared (counted).
const TRACKED: usize = 2048;
/// The smallest buffer shared: below it a copy costs little and there are
/// many (a character's decals).
pub const SMALLEST: u32 = 8 * 1024;

static TABLE: Lock<Table> = Lock::new(Table::new());

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Shared {
    content: Content,
    range: PoolRange,
    /// The buffers that hold the range, 1 or more.
    holders: u32,
}

/// What the table holds, for the memory profile (patch 127).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sharing {
    /// Ranges kept.
    pub ranges: u32,
    /// Buffers that hold a range another buffer holds too.
    pub sharers: u32,
    /// The bytes those would take as copies of their own.
    pub saved: u64,
    pub peak_saved: u64,
    /// New ranges not kept, the table full.
    pub refused: u32,
}

/// What becomes of a range a buffer gives back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Release {
    /// Another buffer holds it: it stays in its chunk.
    Kept,
    /// The last buffer that held it: the pool takes it back.
    Freed,
    /// Not one of ours.
    Unknown,
}

struct Table {
    entries: [Option<Shared>; TRACKED],
    sharing: Sharing,
}

impl Table {
    const fn new() -> Table {
        Table { entries: [None; TRACKED], sharing: Sharing { ranges: 0, sharers: 0, saved: 0, peak_saved: 0, refused: 0 } }
    }

    /// A live range with `content` in it, one more buffer holding it.
    fn give(&mut self, content: Content) -> Option<PoolRange> {
        let shared = self.entries.iter_mut().flatten().find(|shared| shared.content == content)?;
        shared.holders += 1;
        let range = shared.range;
        self.sharing.sharers += 1;
        self.sharing.saved += u64::from(content.bytes());
        self.sharing.peak_saved = self.sharing.peak_saved.max(self.sharing.saved);
        Some(range)
    }

    fn holds(&self, range: PoolRange) -> bool {
        self.entries.iter().flatten().any(|shared| shared.range == range)
    }

    /// A range a buffer has just taken from the pool, with `content` in it.
    fn take(&mut self, range: PoolRange, content: Content) {
        if self.holds(range) {
            return;
        }
        match self.entries.iter_mut().find(|slot| slot.is_none()) {
            Some(slot) => {
                *slot = Some(Shared { content, range, holders: 1 });
                self.sharing.ranges += 1;
            }
            None => self.sharing.refused += 1,
        }
    }

    fn release(&mut self, range: PoolRange) -> Release {
        let Some(slot) = self.entries.iter_mut().find(|slot| slot.is_some_and(|shared| shared.range == range)) else {
            return Release::Unknown;
        };
        let Some(shared) = slot.as_mut() else { return Release::Unknown };
        if shared.holders > 1 {
            shared.holders -= 1;
            self.sharing.sharers -= 1;
            self.sharing.saved -= u64::from(shared.content.bytes());
            return Release::Kept;
        }
        *slot = None;
        self.sharing.ranges -= 1;
        Release::Freed
    }
}

/// The table as it is now.
pub fn sharing(thread: &Thread) -> Sharing {
    TABLE.with(thread, |table| table.sharing)
}

pub fn on_allocate(ctx: &mut Context<'_>) -> Flow<GeometryShareOnAllocateExit> {
    let Some(allocation) = PoolAllocation::at(ctx) else { return Flow::Continue };
    if allocation.bytes() < SMALLEST {
        return Flow::Continue;
    }
    let Some(content) = allocation.content() else { return Flow::Continue };
    match TABLE.with(ctx.thread(), |table| table.give(content)) {
        Some(range) => {
            allocation.give(ctx, range);
            Flow::Exit(GeometryShareOnAllocateExit::Shared)
        }
        None => Flow::Continue,
    }
}

pub fn on_pooled(ctx: &mut Context<'_>) -> Flow<NoExit> {
    let Some(buffer) = PooledBuffer::at(ctx) else { return Flow::Continue };
    let range = buffer.range();
    if range.bytes() < SMALLEST || TABLE.with(ctx.thread(), |table| table.holds(range)) {
        return Flow::Continue;
    }
    if let Some(content) = buffer.content() {
        TABLE.with(ctx.thread(), |table| table.take(range, content));
    }
    Flow::Continue
}

pub fn on_deallocate(ctx: &mut Context<'_>) -> Flow<GeometryShareOnDeallocateExit> {
    let Some(range) = released_range(ctx) else { return Flow::Continue };
    if range.bytes() < SMALLEST {
        return Flow::Continue;
    }
    match TABLE.with(ctx.thread(), |table| table.release(range)) {
        Release::Kept => Flow::Exit(GeometryShareOnDeallocateExit::Kept),
        Release::Freed | Release::Unknown => Flow::Continue,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn content(pool: u32, hash: u32) -> Content {
        Content::made_up(pool, 48, 0x4000, hash)
    }

    fn range(first: u32) -> PoolRange {
        PoolRange::made_up(0x1000, first, first + 0x3FFF)
    }

    #[test]
    fn the_same_bytes_get_the_same_range_until_the_last_holder_gives_it_back() {
        let mut table = Box::new(Table::new());
        assert_eq!(table.give(content(1, 7)), None);
        table.take(range(0), content(1, 7));
        assert_eq!(table.give(content(1, 7)), Some(range(0)));
        assert_eq!(table.give(content(1, 7)), Some(range(0)));
        assert_eq!(table.sharing, Sharing { ranges: 1, sharers: 2, saved: 0x8000, peak_saved: 0x8000, refused: 0 });
        // the constructor sees the range it was given: not a second entry
        table.take(range(0), content(1, 7));
        assert_eq!(table.sharing.ranges, 1);
        assert_eq!(table.release(range(0)), Release::Kept);
        assert_eq!(table.release(range(0)), Release::Kept);
        assert_eq!(table.release(range(0)), Release::Freed);
        assert_eq!(table.release(range(0)), Release::Unknown);
        assert_eq!(table.give(content(1, 7)), None);
        assert_eq!(table.sharing, Sharing { ranges: 0, sharers: 0, saved: 0, peak_saved: 0x8000, refused: 0 });
    }

    #[test]
    fn other_bytes_another_pool_or_stride_are_not_the_same() {
        let mut table = Box::new(Table::new());
        table.take(range(0), content(1, 7));
        assert_eq!(table.give(content(1, 8)), None);
        assert_eq!(table.give(content(2, 7)), None);
        assert_eq!(table.give(Content::made_up(1, 24, 0x4000, 7)), None);
        assert_eq!(table.give(Content::made_up(1, 48, 0x4004, 7)), None);
    }

    #[test]
    fn a_full_table_shares_what_it_has_and_counts_the_rest() {
        let mut table = Box::new(Table::new());
        for at in 0..TRACKED as u32 {
            table.take(range(at * 0x4000), content(1, at));
        }
        table.take(range(u32::MAX - 0x3FFF), content(1, u32::MAX));
        assert_eq!(table.sharing.refused, 1);
        assert_eq!(table.give(content(1, u32::MAX)), None);
        assert_eq!(table.give(content(1, 5)), Some(range(5 * 0x4000)));
        assert_eq!(table.release(range(u32::MAX - 0x3FFF)), Release::Unknown);
    }
}
