//! How much memory the game has left, for patch 126's report
//! (`srp_validate::memory`, `docs/notes/memory-report.md`), read once
//! into its typed values here:
//! - the address space, by `VirtualQuery` from 0 to its end: the free
//!   bytes and the largest free region (the room the video driver has);
//! - the engine's video memory budget, which it computed at its start;
//! - each arena the engine registered (its list, `memory::register_allocator`),
//!   its name and size, and what its allocator's kind tells of its use:
//!   a doug_lea_allocator's heap walked chunk by chunk ([`Mspace::usage`]),
//!   a managed allocator's and the multithreaded region's counts;
//! - in the same `VirtualQuery` walk ([`Walk`]), every allocation by its
//!   type (the census) and the largest outside the arenas, each image's
//!   and mapping's file named (`GetModuleFileNameA`, kernel32's
//!   `K32GetMappedFileNameA`).
//!
//! [`process`] and [`arenas`] are the reader, for any caller on the game's
//! thread; [`snapshot`] is the report's. Patch 127's profiler reads the
//! same arenas with where they lie and their heaps' free blocks
//! ([`placed`]), and the address space region by region ([`regions_map`]).
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
use core::ptr::NonNull;
use srp_validate::memory::{
    self as codec, readable, Arena, Client, Found, Moment, Mspace, Name, Process, Report, Type, Usage, Walk, Walked, MAX_ARENAS, NAME,
};

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

/// The address space from 0 in one walk, the `arenas` (start, size) left
/// out of its largest allocations.
fn walk(arenas: impl IntoIterator<Item = (u32, u32)>) -> Walked {
    let mut walk = Walk::new(arenas);
    regions(0, MAX_REGIONS)
        .for_each(|info| walk.region(codec::Region::of(info.state, info.kind, info.allocation as u32, info.size as u32)));
    walk.end()
}

/// A Windows path's room (MAX_PATH, and more for a long one).
pub(in crate::vostok) const PATH: usize = 0x208;

/// The path of the module loaded at `module` (`GetModuleFileNameA`), in
/// `out`: none when it is no module's. A path longer than `out` is cut.
pub(in crate::vostok) fn module_path(module: NonNull<c_void>, out: &mut [u8; PATH]) -> Option<&[u8]> {
    // SAFETY: GetModuleFileNameA through the exe's import slot; it writes
    // at most `out.len()` bytes into `out`. A handle that is no module's
    // makes it fail (0), nothing more.
    let len = unsafe { (raw::engine_get_module_file_name_a)(module.as_ptr(), out.as_mut_ptr(), out.len() as u32) };
    out.get(..len as usize).filter(|path| !path.is_empty())
}

/// What names an allocation's file: kernel32's K32GetMappedFileNameA
/// (Windows 7 on, and Wine), looked up once a snapshot; none without it.
struct Namer {
    mapped: Option<NonNull<c_void>>,
}

impl Namer {
    fn new() -> Namer {
        // SAFETY: kernel32 is loaded in every process; GetProcAddress of a
        // NUL-ended name gives its export's address or null.
        let mapped = unsafe {
            let kernel32 = (raw::engine_get_module_handle_a)(c"kernel32.dll".as_ptr().cast());
            if kernel32.is_null() {
                None
            } else {
                NonNull::new((raw::engine_get_proc_address)(kernel32, c"K32GetMappedFileNameA".as_ptr().cast()))
            }
        };
        Namer { mapped }
    }

    /// The file of the section mapped at `address` (an image's or a
    /// mapping's view), by its NT path's last part.
    fn mapped(&self, address: u32) -> Option<Name> {
        let function = self.mapped?;
        let mut path = [0u8; PATH];
        // SAFETY: K32GetMappedFileNameA(process, address, buffer, size) of
        // this process (the pseudo-handle -1); it writes at most
        // `path.len()` bytes into `path`, and fails (0) for an address no
        // section's view holds.
        let len = unsafe {
            raw::engine_get_mapped_file_name_a(function, usize::MAX as *mut c_void, address as usize as *const c_void,
                                               path.as_mut_ptr(), path.len() as u32)
        };
        Name::of_file(path.get(..len as usize)?)
    }

    /// The file of the allocation `found`: a module's path for an image
    /// (its section's when that fails), a section's for a mapping, none for
    /// a private one.
    fn name(&self, found: Found) -> Option<Name> {
        match found.kind() {
            Type::Private => None,
            Type::Mapped => self.mapped(found.base()),
            Type::Image => {
                let mut path = [0u8; PATH];
                NonNull::new(found.base() as usize as *mut c_void)
                    .and_then(|module| module_path(module, &mut path).and_then(Name::of_file))
                    .or_else(|| self.mapped(found.base()))
            }
        }
    }
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
        self.usage_with(|_| ())
    }

    /// Its usage, a heap's free blocks told to `free` on the way.
    fn usage_with(&self, free: impl FnMut(u32)) -> Usage {
        // SAFETY: an allocator the engine registered: a static object of
        // the image or one of engine_world's, alive while the game runs;
        // its vtable word tells which kind's fields it has.
        unsafe {
            let Some(allocator) = self.record.allocator() else { return Usage::Unknown };
            match read(allocator.vtable()) {
                raw::image::DOUG_LEA_VTABLE => allocator.mspace().map_or(Usage::Unknown, |mspace| self.heap(mspace, free)),
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
    fn heap(&self, mspace: raw::Mspace, free: impl FnMut(u32)) -> Usage {
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
        state.walk(head, free).unwrap_or(Usage::Unknown)
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

/// An arena of the engine's list with where it lies (patch 127).
pub struct Placed<'t> {
    record: Record,
    thread: core::marker::PhantomData<&'t Thread>,
}

impl Placed<'_> {
    pub fn name(&self) -> Name {
        self.record.name()
    }

    /// Its first byte (0 for one with no place of its own: the process heap).
    pub fn start(&self) -> u32 {
        self.record.start as u32
    }

    /// Its bytes, as registered.
    pub fn size(&self) -> u32 {
        self.record.size()
    }

    /// Its usage as [`arenas`] reads it, a heap's free blocks told to
    /// `free` on the way ([`Mspace::walk`](srp_validate::memory::Mspace::walk):
    /// all of them only when the usage is a heap's).
    pub fn usage_with(&self, free: impl FnMut(u32)) -> Usage {
        self.record.usage_with(free)
    }
}

/// Each arena of the engine's list, in its order, with where it lies.
pub fn placed(_: &Thread) -> impl Iterator<Item = Placed<'_>> + '_ {
    records().map(|record| Placed { record, thread: core::marker::PhantomData })
}

/// A region of the address space, as `VirtualQuery` tells it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Region {
    pub base: u32,
    pub size: u32,
    /// Where the reservation it is part of starts (0 for a free one).
    pub allocation: u32,
    /// MEM_COMMIT 1000h, MEM_RESERVE 2000h, MEM_FREE 10000h.
    pub state: u32,
    /// Its pages' protection (PAGE_GUARD 100h with it: a thread stack's guard).
    pub protect: u32,
    /// MEM_PRIVATE 20000h, MEM_MAPPED 40000h, MEM_IMAGE 1000000h.
    pub kind: u32,
}

/// The address space from 0 region by region, at most `most` of them.
pub fn regions_map(_: &Thread, most: usize) -> impl Iterator<Item = Region> + '_ {
    regions(0, most).map(|info| Region {
        base: info.base as u32,
        size: info.size as u32,
        allocation: info.allocation as u32,
        state: info.state,
        protect: info.protect,
        kind: info.kind,
    })
}

/// The engine's video budget, computed at its start.
fn video_budget() -> u64 {
    // SAFETY: two words of the image, written once at the engine's start.
    unsafe {
        let video = raw::image::video_memory();
        u64::from(read(video.budget_high())) << 32 | u64::from(read(video.budget_low()))
    }
}

/// The process as a whole: its address space and the engine's video
/// budget.
pub fn process(_: &Thread) -> Process {
    walk([]).process(video_budget())
}

/// Each arena of the engine's list, in its order, read as it is now.
pub fn arenas(_: &Thread) -> impl Iterator<Item = Arena> + '_ {
    records().map(|record| Arena { name: record.name(), size: record.size(), usage: record.usage() })
}

/// The snapshot at `moment`: the process, each arena in the engine's
/// order, the address space's census, what the game runs with (`client`),
/// and the largest allocations outside the arenas, named.
pub fn snapshot(thread: &Thread, moment: Moment, client: Client) -> Report {
    let walked = walk(records().map(|record| (record.start as u32, record.size())));
    let mut report = Report::new(moment, walked.process(video_budget()));
    arenas(thread).for_each(|arena| report.add(arena));
    report.add_census(walked.census());
    report.add_client(client);
    let namer = Namer::new();
    walked.largest().for_each(|found| report.add_allocation(found.named(namer.name(found))));
    report
}
