//! How much memory the game has left, for patch 126's report
//! (`srp_validate::memory`, `docs/notes/memory-report.md`), read once
//! into its typed values here:
//! - the address space, by `VirtualQuery` from 0 to its end: the free
//!   bytes and the largest free region (the room the video driver has);
//! - the engine's video memory budget, which it computed at its start;
//! - each arena the engine registered (its list, `memory::register_allocator`),
//!   its name and size, and what its allocator's kind tells of its use:
//!   a doug_lea_allocator's heap walked chunk by chunk ([`Mspace::usage`]),
//!   a managed allocator's and the multithreaded region's counts.
//!
//! [`process`] and [`arenas`] are the reader, for any caller on the game's
//! thread; [`snapshot`] is the report's.
//!
//! Nothing is called in the engine: its own size queries walk a heap
//! another thread may be using with no lock (internal_mallinfo 0xFF200),
//! which can loop for ever on a chunk being split. The walk here reads
//! only memory `VirtualQuery` says is committed and readable, inside the
//! arena (which the engine never frees while it runs), and stops at the
//! first word that is no chunk; such an arena is told as unknown. A walk
//! that ends cleanly can still be slightly stale (another thread may move
//! the top or split a chunk while it runs).

use super::Thread;
use super::world::read;
use crate::raw;
use core::ffi::c_void;
use srp_validate::memory::{readable, Arena, Moment, Mspace, Name, Process, Report, Space, Usage, MAX_ARENAS, NAME};

/// kernel32's `MEMORY_BASIC_INFORMATION` (32-bit: 28 bytes).
#[repr(C)]
#[derive(Default)]
struct MemoryInfo {
    base: usize,
    allocation: usize,
    allocation_protect: u32,
    size: usize,
    state: u32,
    protect: u32,
    kind: u32,
}

/// Regions walked at most (a 32-bit process has some thousands).
const MAX_REGIONS: usize = 1 << 18;
/// Regions of one arena at most (one, as the engine reserves and commits it).
const MAX_ARENA_REGIONS: usize = 64;

/// The region `address` is in.
fn query(address: usize) -> Option<MemoryInfo> {
    let mut info = MemoryInfo::default();
    let size = core::mem::size_of::<MemoryInfo>();
    // SAFETY: VirtualQuery through the exe's import slot, which the loader
    // filled; it writes at most `size` bytes into our own `info`.
    let written = unsafe { (raw::engine_virtual_query)(address as *const c_void, (&raw mut info).cast(), size as u32) };
    (written as usize == size).then_some(info)
}

/// The regions from `start`, each with the next one's start, while
/// VirtualQuery answers and the addresses go up.
fn regions(start: usize, most: usize) -> impl Iterator<Item = MemoryInfo> {
    let mut at = Some(start);
    core::iter::from_fn(move || {
        let info = query(at?)?;
        at = info.base.checked_add(info.size).filter(|&next| next > info.base);
        Some(info)
    })
    .take(most)
}

fn space() -> Space {
    let mut space = Space::default();
    regions(0, MAX_REGIONS).for_each(|info| space.region(info.state, info.size as u32));
    space
}

/// `[start, end)` is all committed and readable.
fn all_readable(start: usize, end: usize) -> bool {
    let mut reached = start;
    for info in regions(start, MAX_ARENA_REGIONS) {
        if !readable(info.state, info.protect) {
            return false;
        }
        reached = info.base.saturating_add(info.size);
        if reached >= end {
            return true;
        }
    }
    reached >= end
}

/// One arena of the engine's list.
struct Record {
    record: raw::ArenaRecord,
    start: usize,
    end: usize,
}

impl Record {
    fn of(record: raw::ArenaRecord) -> Record {
        // SAFETY: a record of the engine's list (from its begin to its
        // end), which only grows at the start: plain words.
        let (low, high, start) = unsafe { (read(record.arena_size_low()), read(record.arena_size_high()), read(record.arena_start())) };
        let size = if high == 0 { low } else { u32::MAX };
        let start = start as usize;
        Record { record, start, end: start.saturating_add(size as usize) }
    }

    fn size(&self) -> u32 {
        self.end.saturating_sub(self.start) as u32
    }

    fn name(&self) -> Name {
        let mut bytes = [0u8; NAME];
        // SAFETY: the record's name, a C string of the image (.rdata):
        // read up to its NUL, at most NAME bytes.
        unsafe {
            if let Some(text) = core::ptr::NonNull::new(self.record.name()) {
                for (at, slot) in bytes.iter_mut().enumerate() {
                    *slot = text.add(at).read();
                    if *slot == 0 {
                        break;
                    }
                }
            }
        }
        Name::of_c(&bytes)
    }

    fn usage(&self) -> Usage {
        // SAFETY: an allocator the engine registered: a static object of
        // the image or one of engine_world's, alive while the game runs;
        // its vtable word tells which kind's fields it has.
        unsafe {
            let Some(allocator) = self.record.allocator() else { return Usage::Unknown };
            match read(allocator.vtable()) {
                raw::image::DOUG_LEA_VTABLE => allocator.mspace().map_or(Usage::Unknown, |mspace| self.heap(mspace)),
                raw::image::MANAGED_VTABLE => Usage::Counted {
                    used: read(allocator.managed_total()).saturating_sub(read(allocator.managed_free())),
                },
                raw::image::PTHREADS_VTABLE => {
                    let region = raw::image::mt_region();
                    Usage::Counted { used: read(region.size()).saturating_sub(read(region.free())) }
                }
                _ => Usage::Unknown,
            }
        }
    }

    /// A doug_lea arena's heap, walked inside the arena alone.
    fn heap(&self, mspace: raw::Mspace) -> Usage {
        let (start, end) = (self.start, self.end);
        let at = mspace.address() as usize;
        let state_end = at.saturating_add(raw::Mspace::SIZE);
        if start == 0 || at < start || state_end > end || !all_readable(start, end) {
            return Usage::Unknown;
        }
        // SAFETY: the mspace's state lies inside the arena, whose bytes are
        // committed and readable and stay so while the game runs; plain
        // words another thread may be writing (a stale one is no fault).
        let state = unsafe {
            Mspace {
                top: read(mspace.top()),
                top_size: read(mspace.top_size()),
                footprint: read(mspace.footprint()),
                segment: read(mspace.segment()),
                segment_size: read(mspace.segment_size()),
                next_segment: read(mspace.next_segment()),
            }
        };
        let head = |chunk: u32| {
            let word = (chunk as usize).checked_add(4)?;
            (word >= start && word.checked_add(4)? <= end).then(|| {
                // SAFETY: a word inside the arena, checked readable above;
                // volatile, as another thread may write it.
                unsafe { (word as *const u32).read_volatile() }
            })
        };
        state.usage(head).unwrap_or(Usage::Unknown)
    }
}

/// The records of the engine's list, at most [`MAX_ARENAS`].
fn records() -> impl Iterator<Item = Record> {
    // SAFETY: the list's slot is the image's; the list itself was made at
    // the engine's start (memory::preinitialize), before any hook.
    let (begin, end) = unsafe {
        match raw::image::arena_list().list() {
            Some(list) => (list.begin(), list.end()),
            None => (None, None),
        }
    };
    let count = match (begin, end) {
        (Some(begin), Some(end)) => end.index_from(begin).unwrap_or(0),
        _ => 0,
    };
    (0..count.min(MAX_ARENAS)).filter_map(move |index| {
        // SAFETY: `index` is below the list's count, from its begin.
        begin.map(|begin| Record::of(unsafe { begin.at(index) }))
    })
}

/// The process as a whole: its address space and the engine's video
/// budget.
pub fn process(_: &Thread) -> Process {
    // SAFETY: two words of the image, written once at the engine's start.
    let video = unsafe {
        let video = raw::image::video_memory();
        u64::from(read(video.budget_high())) << 32 | u64::from(read(video.budget_low()))
    };
    space().process(video)
}

/// Each arena of the engine's list, in its order, read as it is now.
pub fn arenas(_: &Thread) -> impl Iterator<Item = Arena> + '_ {
    records().map(|record| Arena { name: record.name(), size: record.size(), usage: record.usage() })
}

/// The snapshot at `moment`: the process, then each arena in the
/// engine's order.
pub fn snapshot(thread: &Thread, moment: Moment) -> Report {
    let mut report = Report::new(moment, process(thread));
    arenas(thread).for_each(|arena| report.add(arena));
    report
}
