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
//! thread; [`snapshot`] is the report's. Patch 127's profiler reads the
//! same arenas with where they lie and their heaps' free blocks
//! ([`placed`]), and the address space region by region ([`regions_map`]);
//! and what dlmalloc's blocks do not tell: the managed arena's nodes with
//! their resources' classes ([`Placed::managed_nodes`]) and Scaleform's
//! heaps by name ([`scaleform_heaps`]).
//!
//! Nothing is called in the engine: its own size queries walk a heap
//! another thread may be using with no lock (internal_mallinfo 0xFF200),
//! which can loop for ever on a chunk being split. The walk here reads
//! only memory `VirtualQuery` says is committed and readable, inside the
//! arena (which the engine never frees while it runs), and stops at the
//! first word that is no chunk; such an arena is told as unknown. A walk
//! that ends cleanly can still be slightly stale (another thread may move
//! the top or split a chunk while it runs).

use super::{Context, Thread};
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

/// The arenas whose bytes are all committed and readable: where a pointer
/// the game keeps (a node's owner, a Scaleform heap) can be read whatever
/// another thread is doing to the object it names.
struct Readable {
    ranges: [(usize, usize); MAX_ARENAS],
}

impl Readable {
    fn arenas() -> Readable {
        let mut ranges = [(0, 0); MAX_ARENAS];
        for (slot, record) in ranges.iter_mut().zip(records()) {
            if record.start != 0 && all_readable(record.start, record.end) {
                *slot = (record.start, record.end);
            }
        }
        Readable { ranges }
    }

    /// The bytes from `address` to the end of the arena it lies in (0: in none).
    fn span(&self, address: usize) -> usize {
        self.ranges.iter().find(|&&(start, end)| start <= address && address < end).map_or(0, |&(_, end)| end - address)
    }

    fn holds<T>(&self, place: *const T) -> bool {
        self.span(place as usize) >= core::mem::size_of::<T>()
    }
}

/// A resource's class id (`resources::class_id_enum`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ResourceClass(pub u32);

/// A node of a managed arena, its bytes the node's whole (its head too).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManagedNode {
    Free { bytes: u32 },
    /// An allocated one and its resource's class; None when it has no
    /// owner (yet), or one that cannot be read.
    Held { bytes: u32, class: Option<ResourceClass> },
}

/// How a walk of the game's own lists ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Walk {
    /// Nothing of the kind to walk (not a managed arena; no Scaleform heap yet).
    Absent,
    /// To its end: everything was told.
    Whole,
    /// It stopped at a word that did not fit (another thread was changing
    /// the list, or it is not what the walk takes it for): what was told
    /// until then is a part.
    Torn,
}

/// Nodes of a managed arena walked at most (a node is 34h bytes or more).
const MAX_MANAGED_NODES: usize = 1 << 22;

impl Placed<'_> {
    /// A managed arena's nodes, each told to `node` in the arena's order
    /// (patch 127: what the resources (managed) allocator holds, by class).
    /// Nothing of the engine is called and no lock taken: each node is read
    /// inside the arena, checked to tile it, and an owner only inside an
    /// arena that is readable.
    pub fn managed_nodes(&self, mut node: impl FnMut(ManagedNode)) -> Walk {
        // SAFETY: an allocator the engine registered, alive while the game
        // runs (usage_with); its vtable tells its fields are a managed
        // allocator's. Every node is read only inside [start, end), bytes
        // checked readable first, and stays there (the arena is never
        // given back); the words are volatile reads, another thread's
        // writes at worst a stale or torn value, which the checks refuse.
        unsafe {
            let Some(allocator) = self.record.record.allocator() else { return Walk::Absent };
            if read(allocator.vtable()) != raw::image::MANAGED_VTABLE {
                return Walk::Absent;
            }
            let Some(first) = allocator.managed_arena() else { return Walk::Torn };
            let start = first.address() as usize;
            let end = start.saturating_add(read(allocator.managed_total()) as usize);
            if start < self.record.start || end > self.record.end || !all_readable(start, end) {
                return Walk::Torn;
            }
            let readable = Readable::arenas();
            let mut at = first;
            for _ in 0..MAX_MANAGED_NODES {
                let address = at.address() as usize;
                if address < start || address.saturating_add(raw::ManagedNode::SIZE) > end {
                    return Walk::Torn;
                }
                let bytes = read(at.bytes());
                let after = address.saturating_add(bytes as usize);
                if (bytes as usize) < raw::ManagedNode::SIZE || after > end {
                    return Walk::Torn;
                }
                match read(at.kind()) {
                    0 => {
                        // the owner itself inside a readable arena before its resource's word is looked for
                        let owner = at.owner().filter(|owner| readable.span(owner.address() as usize) > 0);
                        let class = owner.map(|owner| owner.class_id()).filter(|&place| readable.holds(place));
                        node(ManagedNode::Held { bytes, class: class.map(|place| ResourceClass(read(place))) });
                    }
                    1 => node(ManagedNode::Free { bytes }),
                    _ => return Walk::Torn,
                }
                match at.next() {
                    None => return if after == end { Walk::Whole } else { Walk::Torn },
                    Some(next) if next.address() as usize == after => at = next,
                    Some(_) => return Walk::Torn,
                }
            }
            Walk::Torn
        }
    }
}

/// A heap of Scaleform's (the Flash UI's memory: every block of it is the
/// "global multithreaded" arena's, taken through xrSysAllocMalloc).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScaleformHeap<'a> {
    /// 0 the global heap, 1 its children (a movie's data, definition, view), and so on.
    pub depth: u8,
    /// Its name as Scaleform keeps it, cut at [`HEAP_NAME`] bytes.
    pub name: &'a [u8],
    /// Its `HeapDesc` flags (1000h: a debug heap).
    pub flags: u32,
    /// The bytes it holds of the system allocator, its children's not counted.
    pub footprint: u32,
    /// The bytes of them in use.
    pub used: u32,
}

/// A heap's name's bytes told at most.
pub const HEAP_NAME: usize = 96;
/// Heaps told at most, and how deep under the global heap.
const MAX_HEAPS: usize = 1024;
const MAX_HEAP_DEPTH: usize = 6;

/// Scaleform's heaps from its global heap down, a parent before its
/// children, each told to `heap` (patch 127). No lock of Scaleform's is
/// taken and nothing of it called: a heap is read only where an arena is
/// readable (Scaleform makes each heap, its engine and its name in one
/// block of the multithreaded arena), so a heap freed under the walk is
/// stale words at worst.
pub fn scaleform_heaps(_: &Thread, mut heap: impl FnMut(ScaleformHeap<'_>)) -> Walk {
    #[derive(Clone, Copy)]
    struct Level {
        at: Option<raw::MemoryHeap>,
        /// Where this level's list ends: its parent's list's root as a heap.
        end: u32,
    }
    // SAFETY: the slot is the image's. Every heap is read only after its
    // bytes are checked to lie in a readable arena, its engine's words and
    // its name's bytes the same; volatile reads of words another thread may write.
    unsafe {
        let Some(global) = raw::image::global_heap().heap() else { return Walk::Absent };
        let readable = Readable::arenas();
        let mut levels = [Level { at: None, end: 0 }; MAX_HEAP_DEPTH];
        levels[0].at = Some(global);
        let mut depth = 0usize;
        for _ in 0..MAX_HEAPS * 2 {
            let level = levels[depth];
            let Some(at) = level.at.filter(|at| at.address() != level.end) else {
                if depth == 0 {
                    return Walk::Whole;
                }
                depth -= 1;
                continue;
            };
            if readable.span(at.address() as usize) < raw::MemoryHeap::SIZE {
                return Walk::Torn;
            }
            let Some(engine) = at.engine() else { return Walk::Torn };
            let (footprint, used) = (engine.footprint(), engine.used());
            if !readable.holds(footprint) || !readable.holds(used) {
                return Walk::Torn;
            }
            let mut name = [0u8; HEAP_NAME];
            let text = at.name();
            let room = readable.span(text as usize).min(HEAP_NAME);
            let mut length = 0;
            while length < room {
                let byte = text.add(length).read_volatile();
                if byte == 0 {
                    break;
                }
                name[length] = byte;
                length += 1;
            }
            heap(ScaleformHeap { depth: depth as u8, name: &name[..length], flags: read(at.flags()), footprint: read(footprint), used: read(used) });
            // the global heap has no siblings; a child's next is its parent's next child
            levels[depth].at = if depth == 0 { None } else { at.next() };
            if depth + 1 < MAX_HEAP_DEPTH {
                depth += 1;
                levels[depth] = Level { at: at.first_child(), end: at.children_end().address() };
            }
        }
        Walk::Torn
    }
}

/// One of the render's two geometry pools: a vertex or index buffer that is
/// neither dynamic nor staging is a range of one of the pool's chunks (each a
/// D3D11 buffer the render made at its start) while a chunk has room for it,
/// else its own D3D11 buffer (untyped_buffer::untyped_buffer 0x3D10F0).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeometryKind {
    Vertices,
    Indices,
}

/// What a geometry pool holds (patch 127), read as [`GeometryKind`]'s pool
/// is now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GeometryPool {
    /// No render resources yet, or no such pool.
    Absent,
    /// A word that did not fit (a vector out of its room, a range out of its
    /// chunk): another thread was changing it.
    Torn,
    Read(PoolUse),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PoolUse {
    pub chunks: u32,
    /// The chunks' bytes.
    pub bytes: u64,
    /// The bytes of the ranges taken.
    pub used: u64,
    pub ranges: u32,
    /// The ranges of the chunk with the most: at 1000h the next one stops the game.
    pub most_ranges: u32,
    /// The bytes of the buffers of its kind that are their own D3D11 buffer.
    pub unpooled: u32,
}

/// A pool's chunks at most, and a chunk's ranges: their vectors' own rooms.
const POOL_CHUNKS: usize = 0x800;
const CHUNK_RANGES: usize = 0x1000;

/// The geometry pool of `kind`. No lock of the render's is taken and
/// nothing of it called: the pool and its chunks are read only in a readable
/// arena (the render made them from its own and keeps them while it runs),
/// each vector only inside its own room.
pub fn geometry_pool(_: &Thread, kind: GeometryKind) -> GeometryPool {
    // SAFETY: the render resources' global, a pointer the game sets once; the
    // pool and every chunk are checked to lie in a readable arena before a
    // word of theirs is read, and each vector's end in its room; volatile
    // reads of words another thread may write, which the checks refuse.
    unsafe {
        let Some(manager) = raw::image::render_resources().manager() else { return GeometryPool::Absent };
        let (pool, unpooled) = match kind {
            GeometryKind::Vertices => (manager.vertex_pool(), read(manager.unpooled_vertex_bytes())),
            GeometryKind::Indices => (manager.index_pool(), read(manager.unpooled_index_bytes())),
        };
        let Some(pool) = pool else { return GeometryPool::Absent };
        let readable = Readable::arenas();
        if readable.span(pool.address() as usize) < raw::HwBufferPool::SIZE {
            return GeometryPool::Torn;
        }
        let (Some(first), Some(end)) = (pool.chunks(), pool.chunks_end()) else { return GeometryPool::Torn };
        let Some(count) = end.index_from(first).filter(|&count| first == pool.chunk_room() && count <= POOL_CHUNKS) else {
            return GeometryPool::Torn;
        };
        let mut usage = PoolUse { unpooled, ..PoolUse::default() };
        for index in 0..count {
            let Some(chunk) = first.at(index).chunk() else { return GeometryPool::Torn };
            if readable.span(chunk.address() as usize) < raw::HwBufferPoolChunk::SIZE {
                return GeometryPool::Torn;
            }
            let bytes = read(chunk.bytes());
            let (Some(begin), Some(end)) = (chunk.ranges(), chunk.ranges_end()) else { return GeometryPool::Torn };
            let Some(ranges) = end.index_from(begin).filter(|&ranges| begin == chunk.range_room() && ranges <= CHUNK_RANGES) else {
                return GeometryPool::Torn;
            };
            for at in 0..ranges {
                let range = begin.at(at);
                let (low, high) = (read(range.first()), read(range.last()));
                if high < low || high >= bytes {
                    return GeometryPool::Torn;
                }
                usage.used += u64::from(high - low) + 1;
            }
            usage.chunks += 1;
            usage.bytes += u64::from(bytes);
            usage.ranges += ranges as u32;
            usage.most_ranges = usage.most_ranges.max(ranges as u32);
        }
        GeometryPool::Read(usage)
    }
}

/// A range of a geometry pool's chunk, as the pool and a pooled buffer keep
/// it: read from the game only (a range given again is one the pool made).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PoolRange {
    chunk: u32,
    first: u32,
    last: u32,
}

impl PoolRange {
    pub fn bytes(&self) -> u32 {
        self.last.wrapping_sub(self.first).wrapping_add(1)
    }

    #[cfg(test)]
    pub fn made_up(chunk: u32, first: u32, last: u32) -> PoolRange {
        PoolRange { chunk, first, last }
    }
}

/// What a pooled buffer holds, as far as another buffer can be told the same:
/// its pool (the vertices' or the indices'), its stride, its bytes' count and
/// two hashes of the bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Content {
    pool: u32,
    stride: u32,
    bytes: u32,
    hash: [u32; 4],
}

impl Content {
    pub fn bytes(&self) -> u32 {
        self.bytes
    }

    #[cfg(test)]
    pub fn made_up(pool: u32, stride: u32, bytes: u32, hash: u32) -> Content {
        Content { pool, stride, bytes, hash: [hash, 0, 0, 0] }
    }
}

/// Two SipHash-2-4 values of `bytes` under two keys: 128 bits.
#[allow(deprecated)] // core's SipHasher: the one keyed hash core has (no heap, no table)
fn content_hash(bytes: &[u8]) -> [u32; 4] {
    use core::hash::{Hasher, SipHasher};
    let hash = |key: u64| {
        let mut hasher = SipHasher::new_with_keys(key, !key);
        hasher.write(bytes);
        hasher.finish()
    };
    let (a, b) = (hash(1), hash(2));
    [a as u32, (a >> 32) as u32, b as u32, (b >> 32) as u32]
}

/// The bytes a buffer is made of, at most this many hashed (a buffer of a
/// pool fits a chunk: 4 MiB).
const CONTENT_LIMIT: u32 = 1 << 22;

/// # Safety
/// `data` is `bytes` readable bytes, alive for the hook.
unsafe fn content(pool: u32, stride: u32, data: *const u8, bytes: u32) -> Option<Content> {
    if data.is_null() || bytes == 0 || bytes > CONTENT_LIMIT || stride == 0 {
        return None;
    }
    let hash = content_hash(unsafe { core::slice::from_raw_parts(data, bytes as usize) });
    Some(Content { pool, stride, bytes, hash })
}

/// hw_buffer_pool::allocate asked for a range (`geometry_share.allocate`):
/// the pool, the bytes to put there, and where the range goes.
pub struct PoolAllocation {
    frame: raw::PoolAllocateFrame,
    pool: u32,
}

impl PoolAllocation {
    pub fn at(ctx: &Context<'_>) -> Option<PoolAllocation> {
        // SAFETY: the context is this site's: allocate's arguments by the
        // site's ESP, alive for the call.
        let (frame, pool) = unsafe {
            let frame = raw::site::geometry_share_allocate::frame(ctx)?;
            (frame, frame.pool()?.address())
        };
        Some(PoolAllocation { frame, pool })
    }

    /// The bytes' count.
    pub fn bytes(&self) -> u32 {
        // SAFETY: allocate's argument, alive for the call.
        unsafe { read(self.frame.bytes()) }
    }

    pub fn content(&self) -> Option<Content> {
        // SAFETY: the bytes allocate would copy into the range (allocate_range's
        // memcpy reads as many), alive for the call.
        unsafe { content(self.pool, read(self.frame.stride()), self.frame.data(), self.bytes()) }
    }

    /// Answers the call with `range`: allocate returns 1 with it filled, the
    /// bytes not copied (they are there). The handler then takes its exit
    /// to allocate's `ret 14h`.
    pub fn give(&self, ctx: &mut Context<'_>, range: PoolRange) {
        // SAFETY: the range allocate fills (the buffer's +4h), 12 bytes.
        unsafe {
            let Some(out) = self.frame.range() else { return };
            out.chunk_address().write_volatile(range.chunk);
            out.first().write_volatile(range.first);
            out.last().write_volatile(range.last);
        }
        ctx.set_reg8(super::Reg::Eax, 1);
    }
}

/// A buffer of the pool's, its range just taken (`geometry_share.pooled`).
pub struct PooledBuffer {
    range: PoolRange,
    pool: u32,
    stride: u32,
    bytes: u32,
    data: *const u8,
}

impl PooledBuffer {
    pub fn at(ctx: &Context<'_>) -> Option<PooledBuffer> {
        // SAFETY: the context is this site's: the buffer being made (EDI) with
        // its range filled, its constructor's frame (EBP) with the bytes it
        // was given, alive for the call; the chunk the pool's.
        unsafe {
            let buffer = raw::site::geometry_share_pooled::buffer(ctx)?;
            let frame = raw::site::geometry_share_pooled::frame(ctx)?;
            let range = range_of(buffer.range())?;
            let pool = buffer.range().chunk()?.pool()?.address();
            Some(PooledBuffer { range, pool, stride: read(buffer.stride()), bytes: read(buffer.bytes()), data: frame.data() })
        }
    }

    pub fn range(&self) -> PoolRange {
        self.range
    }

    /// What it holds: its bytes hashed, now.
    pub fn content(&self) -> Option<Content> {
        // SAFETY: the bytes the buffer was made with, which its pool copied
        // (as many as it has), alive for the constructor's call; `at` is
        // this site's only maker and the value is not Send, so it is used
        // inside the hook.
        unsafe { content(self.pool, self.stride, self.data, self.bytes) }
    }
}

/// A range a buffer gives back (`geometry_share.deallocate`).
pub fn released_range(ctx: &Context<'_>) -> Option<PoolRange> {
    // SAFETY: the context is this site's: EAX the range (the buffer's +4h),
    // alive for the call.
    unsafe { range_of(raw::site::geometry_share_deallocate::range(ctx)?) }
}

/// # Safety
/// `range` is a range's 12 bytes, alive.
unsafe fn range_of(range: raw::HwBufferPoolRange) -> Option<PoolRange> {
    unsafe {
        let chunk = range.chunk()?.address();
        Some(PoolRange { chunk, first: read(range.first()), last: read(range.last()) })
    }
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

#[cfg(test)]
mod tests {
    use super::content_hash;

    #[test]
    fn the_same_bytes_hash_the_same_and_one_byte_more_or_other_does_not() {
        let bytes: Vec<u8> = (0..20_000u32).map(|i| (i * 7 + 3) as u8).collect();
        let mut other = bytes.clone();
        other[12_345] ^= 1;
        assert_eq!(content_hash(&bytes), content_hash(&bytes.clone()));
        assert_ne!(content_hash(&bytes), content_hash(&other));
        assert_ne!(content_hash(&bytes), content_hash(&bytes[..19_999]));
        let [a, b, c, d] = content_hash(&bytes);
        assert!((a, b) != (c, d), "two keys, two values");
    }
}
