//! Virtual memory (VirtualAlloc/Free/Query with a region table), the process heap and the
//! image map used by VirtualQuery.

use std::collections::BTreeMap;
use std::sync::Mutex;

pub const PAGE: usize = 0x1000;
pub const GRAN: usize = 0x10000; // Windows allocation granularity

pub const MEM_COMMIT: u32 = 0x1000;
pub const MEM_RESERVE: u32 = 0x2000;
pub const MEM_DECOMMIT: u32 = 0x4000;
pub const MEM_RELEASE: u32 = 0x8000;
pub const MEM_FREE: u32 = 0x10000;
pub const MEM_PRIVATE: u32 = 0x20000;
pub const MEM_IMAGE: u32 = 0x1000000;

pub const PAGE_NOACCESS: u32 = 0x01;
pub const PAGE_READONLY: u32 = 0x02;
pub const PAGE_READWRITE: u32 = 0x04;
pub const PAGE_WRITECOPY: u32 = 0x08;
pub const PAGE_EXECUTE: u32 = 0x10;
pub const PAGE_EXECUTE_READ: u32 = 0x20;
pub const PAGE_EXECUTE_READWRITE: u32 = 0x40;
pub const PAGE_EXECUTE_WRITECOPY: u32 = 0x80;

pub fn win_to_prot(p: u32) -> i32 {
    match p & 0xff {
        PAGE_NOACCESS => libc::PROT_NONE,
        PAGE_READONLY => libc::PROT_READ,
        PAGE_READWRITE | PAGE_WRITECOPY => libc::PROT_READ | libc::PROT_WRITE,
        PAGE_EXECUTE => libc::PROT_EXEC,
        PAGE_EXECUTE_READ => libc::PROT_READ | libc::PROT_EXEC,
        _ => libc::PROT_READ | libc::PROT_WRITE | libc::PROT_EXEC,
    }
}

/// One VirtualAlloc reservation: per-page protection, 0 = reserved but not committed.
struct Region {
    size: usize,
    alloc_protect: u32,
    pages: Vec<u32>,
}

/// Image layout for VirtualQuery: (start, end, protect) runs, page aligned.
pub struct ImageMap {
    pub base: usize,
    pub size: usize,
    pub runs: Vec<(usize, usize, u32)>,
}

static REGIONS: Mutex<BTreeMap<usize, Region>> = Mutex::new(BTreeMap::new());

// ---------------------------------------------------------------- low address space
// The engine packs pointers into 44 bits in its lock-free structures (e.g. 0x2000D672E), so every address the exe
// can see must stay below 2^44 (16 TB).
// Linux places mmap/heap/stacks near 0x7fxx_xxxx_xxxx (47 bits), so the host hands out memory from
// [LOW_START, LOW_END) only, with MAP_FIXED_NOREPLACE and a bump pointer.
pub const LOW_START: usize = 0x0100_0000_0000; // 1 TB (the image sits at 8 GB)
pub const LOW_END: usize = 0x0FFF_0000_0000;
static LOW_NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(LOW_START);

/// Anonymous mapping below 2^44, 64 KiB aligned. Returns 0 on failure.
pub fn low_mmap(size: usize, prot: i32) -> usize {
    use std::sync::atomic::Ordering;
    let sz = align_up(size.max(1), GRAN);
    loop {
        let a = LOW_NEXT.fetch_add(sz, Ordering::Relaxed);
        if a + sz > LOW_END {
            return 0;
        }
        let p = unsafe {
            libc::mmap(
                a as *mut libc::c_void,
                sz,
                prot,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_NORESERVE | libc::MAP_FIXED_NOREPLACE,
                -1,
                0,
            )
        };
        if p as usize == a {
            mergeable(a, sz);
            return a;
        }
        if p != libc::MAP_FAILED {
            unsafe { libc::munmap(p, sz) };
        }
    }
}
static IMAGE: Mutex<Option<ImageMap>> = Mutex::new(None);

pub fn set_image_map(m: ImageMap) {
    *IMAGE.lock().unwrap() = Some(m);
}

fn align_up(x: usize, a: usize) -> usize {
    (x + a - 1) & !(a - 1)
}

pub unsafe fn virtual_alloc(addr: usize, size: usize, typ: u32, protect: u32) -> usize {
    if size == 0 {
        return 0;
    }
    let mut regs = REGIONS.lock().unwrap();
    if addr != 0 && typ & MEM_RESERVE == 0 {
        // commit inside an existing reservation
        let Some((&base, r)) = regs.range_mut(..=addr).next_back() else { return 0 };
        if addr >= base + r.size {
            return 0;
        }
        let start = addr & !(PAGE - 1);
        let end = align_up(addr + size, PAGE).min(base + r.size);
        if unsafe { libc::mprotect(start as *mut libc::c_void, end - start, win_to_prot(protect)) } != 0 {
            return 0;
        }
        for p in (start - base) / PAGE..(end - base) / PAGE {
            r.pages[p] = protect;
        }
        return start;
    }
    let size_r = align_up(size, PAGE);
    let prot = if typ & MEM_COMMIT != 0 { win_to_prot(protect) } else { libc::PROT_NONE };
    let flags = libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_NORESERVE;
    let base;
    let _ = flags;
    if addr != 0 {
        let a = addr & !(GRAN - 1);
        let sz = align_up(addr + size, PAGE) - a;
        let p = unsafe { libc::mmap(a as *mut libc::c_void, sz, prot, flags | libc::MAP_FIXED_NOREPLACE, -1, 0) };
        if p == libc::MAP_FAILED || p as usize != a {
            return 0;
        }
        mergeable(a, sz);
        base = a;
        regs.insert(base, Region { size: sz, alloc_protect: protect, pages: vec![if typ & MEM_COMMIT != 0 { protect } else { 0 }; sz / PAGE] });
        return base;
    }
    // 64 KiB aligned, below 2^44
    let a = low_mmap(size_r, prot);
    if a == 0 {
        return 0;
    }
    base = a;
    regs.insert(base, Region { size: size_r, alloc_protect: protect, pages: vec![if typ & MEM_COMMIT != 0 { protect } else { 0 }; size_r / PAGE] });
    base
}

pub unsafe fn virtual_free(addr: usize, size: usize, typ: u32) -> bool {
    let mut regs = REGIONS.lock().unwrap();
    if typ & MEM_RELEASE != 0 {
        let Some(r) = regs.remove(&addr) else { return false };
        unsafe { libc::munmap(addr as *mut libc::c_void, r.size) };
        return true;
    }
    if typ & MEM_DECOMMIT != 0 {
        let Some((&base, r)) = regs.range_mut(..=addr).next_back() else { return false };
        let start = addr & !(PAGE - 1);
        let end = if size == 0 { base + r.size } else { align_up(addr + size, PAGE).min(base + r.size) };
        unsafe {
            libc::mmap(start as *mut libc::c_void, end - start, libc::PROT_NONE, libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_FIXED | libc::MAP_NORESERVE, -1, 0);
        }
        // the decommitted range is a new mapping: mark it again (a later commit writes into it)
        mergeable(start, end - start);
        for p in (start - base) / PAGE..(end - base) / PAGE {
            r.pages[p] = 0;
        }
        return true;
    }
    false
}

pub unsafe fn virtual_protect(addr: usize, size: usize, protect: u32) -> Option<u32> {
    let mut regs = REGIONS.lock().unwrap();
    let (&base, r) = regs.range_mut(..=addr).next_back()?;
    if addr >= base + r.size {
        return None;
    }
    let start = addr & !(PAGE - 1);
    let end = align_up(addr + size, PAGE).min(base + r.size);
    let old = r.pages[(start - base) / PAGE];
    unsafe { libc::mprotect(start as *mut libc::c_void, end - start, win_to_prot(protect)) };
    for p in (start - base) / PAGE..(end - base) / PAGE {
        r.pages[p] = protect;
    }
    Some(old)
}

/// MEMORY_BASIC_INFORMATION (48 bytes)
#[repr(C)]
#[derive(Default, Debug, Clone, Copy)]
pub struct Mbi {
    pub base: u64,
    pub alloc_base: u64,
    pub alloc_protect: u32,
    pub partition: u16,
    pub _pad: u16,
    pub region_size: u64,
    pub state: u32,
    pub protect: u32,
    pub typ: u32,
    pub _pad2: u32,
}

fn prot_from_maps(perm: &str) -> u32 {
    let b = perm.as_bytes();
    match (b[0] == b'r', b[1] == b'w', b[2] == b'x') {
        (false, false, false) => PAGE_NOACCESS,
        (true, false, false) => PAGE_READONLY,
        (_, true, false) => PAGE_READWRITE,
        (_, false, true) => PAGE_EXECUTE_READ,
        (_, true, true) => PAGE_EXECUTE_READWRITE,
    }
}

pub fn virtual_query(addr: usize) -> Option<Mbi> {
    let page = addr & !(PAGE - 1);
    if let Some(img) = IMAGE.lock().unwrap().as_ref() {
        if addr >= img.base && addr < img.base + img.size {
            for &(s, e, p) in &img.runs {
                if page >= s && page < e {
                    return Some(Mbi {
                        base: page as u64,
                        alloc_base: img.base as u64,
                        alloc_protect: PAGE_EXECUTE_WRITECOPY,
                        region_size: (e - page) as u64,
                        state: MEM_COMMIT,
                        protect: p,
                        typ: MEM_IMAGE,
                        ..Default::default()
                    });
                }
            }
        }
    }
    {
        let regs = REGIONS.lock().unwrap();
        if let Some((&base, r)) = regs.range(..=addr).next_back() {
            if addr < base + r.size {
                let i = (page - base) / PAGE;
                let p = r.pages[i];
                let mut j = i;
                while j < r.pages.len() && r.pages[j] == p {
                    j += 1;
                }
                return Some(Mbi {
                    base: page as u64,
                    alloc_base: base as u64,
                    alloc_protect: r.alloc_protect,
                    region_size: ((j - i) * PAGE) as u64,
                    state: if p == 0 { MEM_RESERVE } else { MEM_COMMIT },
                    protect: p,
                    typ: MEM_PRIVATE,
                    ..Default::default()
                });
            }
        }
    }
    // anything else: describe the Linux mapping (thread stacks, host heap)
    let maps = std::fs::read_to_string("/proc/self/maps").ok()?;
    let mut prev_end = 0usize;
    for l in maps.lines() {
        let mut it = l.split_whitespace();
        let range = it.next()?;
        let perm = it.next()?;
        let (a, b) = range.split_once('-')?;
        let a = usize::from_str_radix(a, 16).ok()?;
        let b = usize::from_str_radix(b, 16).ok()?;
        if addr >= a && addr < b {
            return Some(Mbi {
                base: page as u64,
                alloc_base: a as u64,
                alloc_protect: PAGE_READWRITE,
                region_size: (b - page) as u64,
                state: MEM_COMMIT,
                protect: prot_from_maps(perm),
                typ: MEM_PRIVATE,
                ..Default::default()
            });
        }
        if addr >= prev_end && addr < a {
            return Some(Mbi { base: page as u64, region_size: (a - page) as u64, state: MEM_FREE, protect: PAGE_NOACCESS, ..Default::default() });
        }
        prev_end = b;
    }
    if addr < 0x7FFF_FFFF_0000 {
        return Some(Mbi { base: page as u64, region_size: (0x7FFF_FFFF_0000 - page) as u64, state: MEM_FREE, protect: PAGE_NOACCESS, ..Default::default() });
    }
    None
}

// ---------------------------------------------------------------- heap
// Own allocator over low_mmap (glibc malloc returns 47-bit addresses). Blocks carry a 16-byte header
// (requested size, tag|class) so HeapSize is exact; user pointers are 16-byte aligned.
// Size classes: multiples of 16 up to 1 KiB, then 4 steps per power of two up to 1 MiB; larger
// blocks get their own mapping.

const TAG: u64 = 0x4845_4150_0000_0000;
const BIG: u64 = 0xFFFF;
struct Heap {
    free: Vec<Vec<usize>>,
    chunk_next: usize,
    chunk_end: usize,
}
static HEAP: Mutex<Option<Heap>> = Mutex::new(None);

fn class_of(total: usize) -> Option<(usize, usize)> {
    if total <= 1024 {
        let c = total.div_ceil(16);
        return Some((c, c * 16));
    }
    if total > 1 << 20 {
        return None;
    }
    let mut idx = 64;
    let mut p = 1024usize;
    loop {
        for k in 1..=4 {
            let sz = p + p / 4 * k;
            idx += 1;
            if total <= sz {
                return Some((idx, sz));
            }
        }
        p *= 2;
    }
}

/// Bytes a zeroing request (HEAP_ZERO_MEMORY, LMEM/GMEM_ZEROINIT, a zeroing realloc's tail) did not write because
/// the block was fresh: the memory-saving trace of `MEM [...] savings`.
static ZERO_SKIPPED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// The zeroing rule of the heap. A fresh block was never handed out before: a big block is a new anonymous mapping
/// (`low_mmap` never returns an address twice: `LOW_NEXT` only grows), a class block is bump space of a chunk that
/// nothing wrote (blocks go back to the free lists, never to the chunk). Both still hold the zeros of the mapping, so
/// writing zeros changes no byte and only dirties pages. A block popped from a free list is reused and is zeroed
/// whenever the caller asks for zeroed memory.
pub(crate) const fn needs_zeroing(zero: bool, fresh: bool) -> bool {
    zero && !fresh
}

pub unsafe fn heap_alloc(size: usize, zero: bool) -> usize {
    unsafe { heap_alloc_fresh(size, zero).0 }
}

/// `heap_alloc` that also says whether the block is fresh (see [`needs_zeroing`]). (0, false) on failure.
unsafe fn heap_alloc_fresh(size: usize, zero: bool) -> (usize, bool) {
    let total = size + 16;
    let (block, cls, fresh) = match class_of(total) {
        Some((c, sz)) => {
            let mut g = HEAP.lock().unwrap();
            let h = g.get_or_insert_with(|| Heap { free: vec![Vec::new(); 200], chunk_next: 0, chunk_end: 0 });
            if let Some(b) = h.free[c].pop() {
                (b, c as u64, false)
            } else {
                if h.chunk_next + sz > h.chunk_end {
                    let n = 64 << 20;
                    let a = low_mmap(n, libc::PROT_READ | libc::PROT_WRITE);
                    if a == 0 {
                        return (0, false);
                    }
                    crate::pagemap::record(crate::pagemap::HEAP_CHUNK, a, n);
                    h.chunk_next = a;
                    h.chunk_end = a + n;
                }
                let b = h.chunk_next;
                h.chunk_next += sz;
                (b, c as u64, true)
            }
        }
        None => {
            let a = low_mmap(total, libc::PROT_READ | libc::PROT_WRITE);
            if a == 0 {
                return (0, false);
            }
            crate::pagemap::record(crate::pagemap::HEAP_BIG, a, total);
            (a, BIG, true)
        }
    };
    unsafe {
        *(block as *mut u64) = size as u64;
        *((block + 8) as *mut u64) = TAG | cls;
        if needs_zeroing(zero, fresh) {
            std::ptr::write_bytes((block + 16) as *mut u8, 0, size);
        } else if zero {
            ZERO_SKIPPED.fetch_add(size as u64, std::sync::atomic::Ordering::Relaxed);
        }
    }
    (block + 16, fresh)
}

pub unsafe fn heap_size(ptr: usize) -> usize {
    if ptr == 0 {
        return usize::MAX;
    }
    unsafe { *((ptr - 16) as *const u64) as usize }
}

pub unsafe fn heap_free(ptr: usize) -> bool {
    if ptr == 0 {
        return true;
    }
    let block = ptr - 16;
    let t = unsafe { *((block + 8) as *const u64) };
    if t & 0xFFFF_FFFF_0000_0000 != TAG {
        crate::hlog!("HeapFree: bad block {ptr:#x} (tag {t:#x})");
        return false;
    }
    unsafe { *((block + 8) as *mut u64) = 0 };
    let cls = t & 0xFFFF;
    if cls == BIG {
        let size = unsafe { *(block as *const u64) } as usize + 16;
        unsafe { libc::munmap(block as *mut libc::c_void, align_up(size, GRAN)) };
    } else {
        HEAP.lock().unwrap().as_mut().unwrap().free[cls as usize].push(block);
    }
    true
}

pub unsafe fn heap_realloc(ptr: usize, size: usize, zero: bool, in_place_only: bool) -> usize {
    if ptr == 0 {
        return unsafe { heap_alloc(size, zero) };
    }
    let old = unsafe { heap_size(ptr) };
    if in_place_only {
        if size <= old {
            unsafe { *((ptr - 16) as *mut u64) = size as u64 };
            return ptr;
        }
        return 0;
    }
    let (n, fresh) = unsafe { heap_alloc_fresh(size, false) };
    if n == 0 {
        return 0;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(ptr as *const u8, n as *mut u8, old.min(size));
        if zero && size > old {
            // the tail past the copied bytes: a fresh block still holds zeros there
            if needs_zeroing(zero, fresh) {
                std::ptr::write_bytes((n + old) as *mut u8, 0, size - old);
            } else {
                ZERO_SKIPPED.fetch_add((size - old) as u64, std::sync::atomic::Ordering::Relaxed);
            }
        }
        heap_free(ptr);
    }
    n
}

// ---------------------------------------------------------------- page pool
// Objects that are never freed (the null cooks' empty resources), bump-allocated from 64 MiB low chunks: a size under
// one page takes a 16-aligned slot of its size rounded up to 16, a larger one whole pages, page aligned. Fresh
// anonymous memory holds zeros and no byte is handed out twice, so nothing is written here; a page costs memory only
// once touched. Through the heap every such object (0x1000 bytes + the 16-byte header = the 5120-byte class) took
// 5120 bytes.

static POOL: Mutex<(usize, usize)> = Mutex::new((0, 0));
static POOL_OBJECTS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static POOL_BYTES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// Bytes of the slots that can be resident: a slot of a page or more counts one page (only page 0 is touched, class
/// 0x14: nullcook.rs).
static POOL_TOUCHED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
const POOL_CHUNK: usize = 64 << 20;

/// The host allocator's locks (page pool, heap, VirtualAlloc regions), held across fork() by `run::fork_world`, so no
/// other thread is inside one of them at the fork instant (a copy locked by a thread the child does not have would
/// stay locked there). None of the three is taken while another one is held (`low_mmap` takes none).
pub struct Held {
    _pool: std::sync::MutexGuard<'static, (usize, usize)>,
    _heap: std::sync::MutexGuard<'static, Option<Heap>>,
    _regions: std::sync::MutexGuard<'static, BTreeMap<usize, Region>>,
}

pub fn hold() -> Held {
    Held {
        _pool: POOL.lock().unwrap_or_else(|e| e.into_inner()),
        _heap: HEAP.lock().unwrap_or_else(|e| e.into_inner()),
        _regions: REGIONS.lock().unwrap_or_else(|e| e.into_inner()),
    }
}

/// `size` fresh zero bytes below 2^44: 16-aligned (page aligned from one page up), never handed out before, never
/// freed. 0 on failure.
pub fn pool_alloc(size: usize) -> usize {
    let (align, len) = if size >= PAGE { (PAGE, size.div_ceil(PAGE) * PAGE) } else { (16, size.max(16).div_ceil(16) * 16) };
    if len > POOL_CHUNK {
        return 0;
    }
    let mut g = POOL.lock().unwrap();
    let mut p = g.0.next_multiple_of(align);
    if g.1 == 0 || p + len > g.1 {
        let a = low_mmap(POOL_CHUNK, libc::PROT_READ | libc::PROT_WRITE);
        if a == 0 {
            return 0;
        }
        crate::pagemap::record(crate::pagemap::POOL_CHUNK, a, POOL_CHUNK);
        *g = (a, a + POOL_CHUNK);
        p = a;
    }
    g.0 = p + len;
    let rel = std::sync::atomic::Ordering::Relaxed;
    POOL_OBJECTS.fetch_add(1, rel);
    POOL_BYTES.fetch_add(len as u64, rel);
    POOL_TOUCHED.fetch_add(len.min(PAGE) as u64, rel);
    p
}

/// The layout of a guarded slot ([`pool_alloc_guarded`]) for an object of `size` bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GuardSlot {
    /// pages of the slot, the inaccessible last one included
    pub pages: usize,
    /// where the object starts in the slot: it ends where the inaccessible page starts
    pub obj_off: usize,
    /// where the inaccessible page starts in the slot
    pub guard_off: usize,
}

/// The guarded slot of an object of `size` bytes: whole accessible pages with the object at their end, then one
/// inaccessible page. The bytes before the object (less than one page) are the slot's fill.
pub const fn guard_slot(size: usize) -> GuardSlot {
    let rw = size.div_ceil(PAGE);
    GuardSlot { pages: rw + 1, obj_off: rw * PAGE - size, guard_off: rw * PAGE }
}

/// The byte the fill before a guarded object holds (a write there shows later as a changed byte).
pub const GUARD_FILL: u8 = 0xA5;

/// Diagnosis only (`nullcook`, HOST_NULL_RES_GUARD=right): `size` fresh zero bytes from the pool that end exactly where
/// an inaccessible page starts, so any access past `size` faults at once; the bytes before the object in its first
/// page hold [`GUARD_FILL`]. Returns the object's address, or 0 (nothing taken) when no chunk can be mapped or the page
/// cannot be protected. Not counted in the savings.
pub fn pool_alloc_guarded(size: usize) -> usize {
    let s = guard_slot(size);
    let len = s.pages * PAGE;
    if size == 0 || len > POOL_CHUNK {
        return 0;
    }
    let mut g = POOL.lock().unwrap();
    let mut p = g.0.next_multiple_of(PAGE);
    if g.1 == 0 || p + len > g.1 {
        let a = low_mmap(POOL_CHUNK, libc::PROT_READ | libc::PROT_WRITE);
        if a == 0 {
            return 0;
        }
        crate::pagemap::record(crate::pagemap::POOL_CHUNK, a, POOL_CHUNK);
        *g = (a, a + POOL_CHUNK);
        p = a;
    }
    if unsafe { libc::mprotect((p + s.guard_off) as *mut libc::c_void, PAGE, libc::PROT_NONE) } != 0 {
        return 0;
    }
    g.0 = p + len;
    unsafe { std::ptr::write_bytes(p as *mut u8, GUARD_FILL, s.obj_off) };
    p + s.obj_off
}

/// The VirtualAlloc regions (start, end), for the page-map diagnosis (`pagemap`).
pub fn virtual_alloc_regions() -> Vec<(usize, usize)> {
    REGIONS.lock().unwrap().iter().map(|(&a, r)| (a, a + r.size)).collect()
}

/// A fresh zero page (`pool_alloc(PAGE)`).
pub fn pool_page() -> usize {
    pool_alloc(PAGE)
}

/// Bytes the pool saved against the heap blocks the null cooks took before (one 5120-byte block per object).
fn pool_saving() -> u64 {
    let rel = std::sync::atomic::Ordering::Relaxed;
    let heap = class_of(PAGE + 16).map(|(_, sz)| sz).unwrap_or(PAGE) as u64;
    (POOL_OBJECTS.load(rel) * heap).saturating_sub(POOL_TOUCHED.load(rel))
}

// ---------------------------------------------------------------- zero pages (start-up)
// The exe's static ctor 0x200075B70 runs 0x2000AA770 on 27 (0x1B) per-thread job-slot arrays at
// 0x20411B0F0 + i * 0x400008. 0x2000AA770 calls 0x2000AF590 on 0x10000 slots of 0x40 bytes (it stores only zeros:
// +0, +0x30, +0x38, +0x3C) and stores 0 at +0x400000. That leaves 108 MiB of BSS that holds zeros but is resident and
// private (dirty). A thread takes array i at its registration (0x200092196..0x2000921AA, 0x2000ACFDD..0x2000ACFF1).
// After the drop a read of a dropped page gives zeros (zero-fill-on-demand of a private anonymous mapping, madvise(2)),
// exactly the bytes it held, and a later write allocates the page again, as the copy-on-write of a shared zero page
// in a fork child did before.

pub const JOB_ARRAYS: usize = 0x2_0411_B0F0;
pub const JOB_ARRAY_STRIDE: usize = 0x40_0008;
pub const JOB_ARRAY_COUNT: usize = 0x1B;

/// Pages dropped by [`drop_startup_zero_pages`] (the trace of `MEM [...] savings`).
static ZERO_DROPPED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Result of one zero-page drop: resident pages read, all-zero pages given back.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZeroDrop {
    pub scanned: usize,
    pub dropped: usize,
}

/// True when the mappings of `maps` (the /proc/self/maps text) cover [lo, hi) without a gap and every one of them is
/// readable, private and anonymous (no file, device 00:00, inode 0). MADV_DONTNEED reads back zeros only there: a
/// private file mapping would read the file's bytes again and a shared mapping would keep its contents.
pub fn maps_private_anon_readable(maps: &str, lo: usize, hi: usize) -> bool {
    let mut cur = lo;
    for l in maps.lines() {
        let mut it = l.split_whitespace();
        let (Some(range), Some(perm), Some(_off), Some(dev), Some(inode)) = (it.next(), it.next(), it.next(), it.next(), it.next()) else {
            return false;
        };
        let name = it.next().unwrap_or("");
        let Some((a, b)) = range.split_once('-') else { return false };
        let (Ok(a), Ok(b)) = (usize::from_str_radix(a, 16), usize::from_str_radix(b, 16)) else { return false };
        if b <= cur || a >= hi {
            continue;
        }
        if a > cur {
            return false; // a hole in the range
        }
        let p = perm.as_bytes();
        let anon = dev == "00:00" && inode == "0" && (name.is_empty() || name.starts_with("[anon:"));
        if p.len() < 4 || p[0] != b'r' || p[3] != b'p' || !anon {
            return false;
        }
        cur = b;
        if cur >= hi {
            return true;
        }
    }
    cur >= hi
}

unsafe fn page_is_zero(a: usize) -> bool {
    let w = unsafe { std::slice::from_raw_parts(a as *const u64, PAGE / 8) };
    w.iter().all(|&x| x == 0)
}

unsafe fn dontneed(a: usize, pages: usize) -> usize {
    if unsafe { libc::madvise(a as *mut libc::c_void, pages * PAGE, libc::MADV_DONTNEED) } == 0 {
        pages
    } else {
        0
    }
}

/// Give back every resident page of [lo, hi) (whole pages inside the range only) whose 4096 bytes are all zero, with
/// MADV_DONTNEED. Refuses a range that is not readable private anonymous memory. The caller makes sure that no other
/// thread can write the range during the call (a write between the check and the drop would be lost).
pub unsafe fn drop_zero_pages(lo: usize, hi: usize) -> Result<ZeroDrop, String> {
    let lo = align_up(lo, PAGE);
    let hi = hi & !(PAGE - 1);
    let mut out = ZeroDrop::default();
    if hi <= lo {
        return Ok(out);
    }
    let maps = std::fs::read_to_string("/proc/self/maps").map_err(|e| format!("/proc/self/maps: {e}"))?;
    if !maps_private_anon_readable(&maps, lo, hi) {
        return Err(format!("{lo:#x}..{hi:#x} is not readable private anonymous memory"));
    }
    let n = (hi - lo) / PAGE;
    // resident pages only (a page never touched costs nothing); without mincore every page is read
    let mut res = vec![0u8; n];
    let have = unsafe { libc::mincore(lo as *mut libc::c_void, hi - lo, res.as_mut_ptr()) } == 0;
    let mut start: Option<usize> = None;
    for (i, r) in res.iter().enumerate() {
        let resident = !have || r & 1 != 0;
        let zero = if resident {
            out.scanned += 1;
            unsafe { page_is_zero(lo + i * PAGE) }
        } else {
            false
        };
        if zero {
            if start.is_none() {
                start = Some(i);
            }
            continue;
        }
        if let Some(s) = start.take() {
            out.dropped += unsafe { dontneed(lo + s * PAGE, i - s) };
        }
    }
    if let Some(s) = start.take() {
        out.dropped += unsafe { dontneed(lo + s * PAGE, n - s) };
    }
    Ok(out)
}

/// M10: no transparent huge pages for this process (`PR_SET_THP_DISABLE`, inherited by fork() children and kept
/// across execve). The image, the exe's 3 GB reserved arena and the low mappings are touched sparsely: on a kernel
/// whose THP mode is "always", one touched 4 KB page can be backed by a whole 2 MB page, and khugepaged collapses
/// partly touched ranges into 2 MB pages, which the zero-page drops and the fork's copy-on-write then work on whole.
/// Under the "madvise" mode nothing changes: no mapping here asks for huge pages. Returns whether the kernel took it.
pub fn disable_huge_pages() -> bool {
    unsafe { libc::prctl(libc::PR_SET_THP_DISABLE, 1 as libc::c_ulong, 0 as libc::c_ulong, 0 as libc::c_ulong, 0 as libc::c_ulong) == 0 }
}

/// The system's THP mode (the bracketed word of /sys/kernel/mm/transparent_hugepage/enabled), or "unknown".
pub fn thp_mode() -> String {
    std::fs::read_to_string("/sys/kernel/mm/transparent_hugepage/enabled")
        .ok()
        .and_then(|s| thp_mode_of(&s))
        .unwrap_or_else(|| "unknown".to_string())
}

fn thp_mode_of(s: &str) -> Option<String> {
    let a = s.find('[')?;
    let b = s[a..].find(']')? + a;
    Some(s[a + 1..b].to_string())
}

/// How this process takes part in the kernel's same-page merging (KSM), [`enable_memory_merge`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MemoryMerge {
    /// `PR_SET_MEMORY_MERGE` (kernel 6.4+): every anonymous mapping, inherited by fork() children.
    Process,
    /// `PR_SET_MEMORY_MERGE` refused (the errno; an older kernel): the exe's own mappings get `MADV_MERGEABLE`
    /// (the image, VirtualAlloc regions, the low mappings), which fork() children inherit too.
    PerMapping(i32),
}

static MERGE_PER_MAPPING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// KSM for this process: identical pages of the base, its zygotes and their matches (the same bot kits in every match
/// of a PvE mission, level data that variants of a level share, untouched copies) can be merged by the kernel's ksmd
/// into one copy, copied again on the first write: no byte any process reads changes. The kernel merges only while
/// `/sys/kernel/mm/ksm/run` is 1 (the operator's choice, docs/guide-server.md); otherwise this only marks the memory.
/// Called once at process start, before the first large mapping.
pub fn enable_memory_merge() -> MemoryMerge {
    let r = unsafe { libc::prctl(libc::PR_SET_MEMORY_MERGE, 1 as libc::c_ulong, 0 as libc::c_ulong, 0 as libc::c_ulong, 0 as libc::c_ulong) };
    if r == 0 {
        return MemoryMerge::Process;
    }
    let e = std::io::Error::last_os_error().raw_os_error().unwrap_or(0);
    MERGE_PER_MAPPING.store(true, std::sync::atomic::Ordering::Relaxed);
    MemoryMerge::PerMapping(e)
}

/// `MADV_MERGEABLE` on a new mapping when the process-wide switch was refused ([`MemoryMerge::PerMapping`]).
pub fn mergeable(a: usize, len: usize) {
    if MERGE_PER_MAPPING.load(std::sync::atomic::Ordering::Relaxed) && len != 0 {
        unsafe { libc::madvise(a as *mut libc::c_void, len, libc::MADV_MERGEABLE) };
    }
}

/// The system's KSM switch (`/sys/kernel/mm/ksm/run`: 0 off, 1 merging, 2 unmerge), or "unknown" (no KSM).
pub fn ksm_run() -> String {
    std::fs::read_to_string("/sys/kernel/mm/ksm/run").map(|s| s.trim().to_string()).unwrap_or_else(|_| "unknown".into())
}

/// Pages given back by [`drop_range`] (the render command pool's unused tail).
static RANGE_DROPPED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Give back every whole page of [lo, hi) with MADV_DONTNEED, whatever it holds: the pages read back as zeros. Refuses
/// a range that is not readable private anonymous memory. The caller makes sure that nothing uses the range's contents
/// again. Returns the pages given back.
pub unsafe fn drop_range(lo: usize, hi: usize) -> Result<usize, String> {
    let lo = align_up(lo, PAGE);
    let hi = hi & !(PAGE - 1);
    if hi <= lo {
        return Ok(0);
    }
    let maps = std::fs::read_to_string("/proc/self/maps").map_err(|e| format!("/proc/self/maps: {e}"))?;
    if !maps_private_anon_readable(&maps, lo, hi) {
        return Err(format!("{lo:#x}..{hi:#x} is not readable private anonymous memory"));
    }
    let n = unsafe { dontneed(lo, (hi - lo) / PAGE) };
    if n == 0 {
        return Err(format!("madvise({lo:#x}, {} pages, MADV_DONTNEED) failed", (hi - lo) / PAGE));
    }
    RANGE_DROPPED.fetch_add(n as u64, std::sync::atomic::Ordering::Relaxed);
    Ok(n)
}

/// M2: drop the all-zero pages of the exe's writable image, first the 27 job-slot arrays, then the rest of .data/BSS.
/// Called at the end of step 1 (CRT, static ctors, engine_world ctor), before any fork: the world thread is then the
/// only thread that runs exe code (the first CreateThread comes in step 2's engine init), the host's other thread
/// waits in pthread_join, and the image is one private anonymous mapping (run::load). Skipped, with a log line, when
/// an exe thread exists already.
pub fn drop_startup_zero_pages() {
    let threads = crate::k32::EXE_THREADS.lock().unwrap().len();
    if threads != 0 {
        crate::hlog!("zero-page drop skipped: {threads} exe threads run already (it is safe only while one thread runs exe code)");
        return;
    }
    let jobs = (JOB_ARRAYS, JOB_ARRAYS + JOB_ARRAY_COUNT * JOB_ARRAY_STRIDE);
    let mut regions: Vec<(&str, usize, usize)> = vec![("job-slot arrays", jobs.0, jobs.1)];
    if let Some(m) = IMAGE.lock().unwrap().as_ref() {
        for &(a, b, p) in &m.runs {
            if p == PAGE_READWRITE {
                regions.push(("other writable image (.data/BSS)", a, b));
            }
        }
    }
    let mut sum: BTreeMap<&str, ZeroDrop> = BTreeMap::new();
    for (what, a, b) in regions {
        match unsafe { drop_zero_pages(a, b) } {
            Ok(d) => {
                let e = sum.entry(what).or_default();
                e.scanned += d.scanned;
                e.dropped += d.dropped;
            }
            Err(e) => crate::hlog!("zero-page drop: {what} {a:#x}..{b:#x} skipped: {e}"),
        }
    }
    // the job arrays go first: their pages are no longer resident when the whole image is read, so the second line
    // counts the rest only
    for what in ["job-slot arrays", "other writable image (.data/BSS)"] {
        if let Some(d) = sum.get(what) {
            ZERO_DROPPED.fetch_add(d.dropped as u64, std::sync::atomic::Ordering::Relaxed);
            crate::hlog!(
                "zero-page drop (step 1, one exe thread): {what}: {} resident pages read, {} all-zero pages given back ({} kB saved)",
                d.scanned,
                d.dropped,
                d.dropped * PAGE / 1024
            );
        }
    }
}

/// The memory-saving trace for `MEM [...]` lines: totals of this process (a fork child counts its zygote's part too).
pub fn savings_summary() -> String {
    let rel = std::sync::atomic::Ordering::Relaxed;
    let skipped = ZERO_SKIPPED.load(rel);
    let dropped = ZERO_DROPPED.load(rel);
    format!(
        "zero pages given back {} kB; zeroing of fresh heap memory skipped {} kB; null-resource pool {} objects in {} kB ({} kB saved against heap blocks); render command pool tail given back {} kB",
        dropped * PAGE as u64 / 1024,
        skipped / 1024,
        POOL_OBJECTS.load(rel),
        POOL_BYTES.load(rel) / 1024,
        pool_saving() / 1024,
        RANGE_DROPPED.load(rel) * PAGE as u64 / 1024
    )
}

// ---------------------------------------------------------------- threads with low stacks

struct Start(Box<dyn FnOnce() + Send>);

extern "C" fn thread_entry(p: *mut libc::c_void) -> *mut libc::c_void {
    let b = unsafe { Box::from_raw(p as *mut Start) };
    (b.0)();
    std::ptr::null_mut()
}

/// pthread with a stack below 2^44 (exe code keeps stack addresses in packed pointers too).
pub fn spawn_low(stack: usize, f: Box<dyn FnOnce() + Send>) -> libc::pthread_t {
    let stack = align_up(stack.max(1 << 20), GRAN);
    let base = low_mmap(stack, libc::PROT_READ | libc::PROT_WRITE);
    assert!(base != 0, "no low stack");
    crate::pagemap::record(crate::pagemap::LOW_STACK, base, stack);
    unsafe {
        let mut attr: libc::pthread_attr_t = std::mem::zeroed();
        libc::pthread_attr_init(&mut attr);
        libc::pthread_attr_setstack(&mut attr, base as *mut libc::c_void, stack);
        let mut t: libc::pthread_t = 0;
        let arg = Box::into_raw(Box::new(Start(f)));
        let r = libc::pthread_create(&mut t, &attr, thread_entry, arg as *mut libc::c_void);
        assert_eq!(r, 0, "pthread_create");
        libc::pthread_attr_destroy(&mut attr);
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_thp_mode_is_the_bracketed_word() {
        assert_eq!(thp_mode_of("always [madvise] never\n").as_deref(), Some("madvise"));
        assert_eq!(thp_mode_of("[always] madvise never\n").as_deref(), Some("always"));
        assert_eq!(thp_mode_of("always madvise never\n"), None);
        assert_eq!(thp_mode_of(""), None);
    }

    fn resident(a: usize, pages: usize) -> Vec<bool> {
        let mut v = vec![0u8; pages];
        assert_eq!(unsafe { libc::mincore(a as *mut libc::c_void, pages * PAGE, v.as_mut_ptr()) }, 0);
        v.iter().map(|x| x & 1 != 0).collect()
    }

    fn anon(pages: usize, prot: i32) -> usize {
        let a = unsafe { libc::mmap(std::ptr::null_mut(), pages * PAGE, prot, libc::MAP_PRIVATE | libc::MAP_ANONYMOUS, -1, 0) };
        assert_ne!(a, libc::MAP_FAILED);
        // 4 KB pages only (no huge page over the test buffer, no merge with a neighbouring mapping)
        unsafe { libc::madvise(a, pages * PAGE, libc::MADV_NOHUGEPAGE) };
        a as usize
    }

    #[test]
    fn zero_page_drop_gives_back_only_all_zero_pages_and_keeps_every_byte() {
        let n = 8;
        let a = anon(n, libc::PROT_READ | libc::PROT_WRITE);
        unsafe {
            // pages 0..=6 written (resident): 1 and 5 hold one non-zero byte each; page 7 never touched
            std::ptr::write_bytes(a as *mut u8, 0, 7 * PAGE);
            *((a + PAGE + 100) as *mut u8) = 7;
            *((a + 6 * PAGE - 1) as *mut u8) = 9;
        }
        assert_eq!(resident(a, n), [true, true, true, true, true, true, true, false]);
        let d = unsafe { drop_zero_pages(a, a + n * PAGE) }.unwrap();
        assert_eq!(d, ZeroDrop { scanned: 7, dropped: 5 });
        assert_eq!(resident(a, n), [false, true, false, false, false, true, false, false]);
        // every byte reads as before
        let bytes = unsafe { std::slice::from_raw_parts(a as *const u8, n * PAGE) };
        for (i, &b) in bytes.iter().enumerate() {
            let want = if i == PAGE + 100 {
                7
            } else if i == 6 * PAGE - 1 {
                9
            } else {
                0
            };
            assert_eq!(b, want, "byte {i:#x}");
        }
        // a page given back is writable as before
        unsafe { *((a + 3 * PAGE + 8) as *mut u8) = 1 };
        assert_eq!(unsafe { *((a + 3 * PAGE + 8) as *const u8) }, 1);
        // only whole pages inside the range: a range inside one page drops nothing
        let d = unsafe { drop_zero_pages(a + 2 * PAGE + 16, a + 3 * PAGE - 16) }.unwrap();
        assert_eq!(d, ZeroDrop::default());
        unsafe { libc::munmap(a as *mut libc::c_void, n * PAGE) };
    }

    #[test]
    fn zero_page_drop_refuses_memory_it_cannot_read_back_as_zeros() {
        let a = anon(2, libc::PROT_NONE);
        assert!(unsafe { drop_zero_pages(a, a + 2 * PAGE) }.is_err());
        unsafe { libc::munmap(a as *mut libc::c_void, 2 * PAGE) };
        let maps = "\
200000000-200001000 r--p 00000000 00:00 0 \n\
200001000-2008a6000 r-xp 00000000 00:00 0 \n\
200a90000-215d9e000 rw-p 00000000 00:00 0 \n\
300000000-300010000 rw-s 00000000 00:01 1234 /dev/zero (deleted)\n\
400000000-400010000 rw-p 00000000 08:01 777 /usr/lib/x.so\n\
500000000-500010000 ---p 00000000 00:00 0 \n\
600000000-600010000 rw-p 00000000 00:00 0 [anon:x]\n";
        assert!(maps_private_anon_readable(maps, 0x200000000, 0x2008a6000), "two mappings without a hole");
        assert!(maps_private_anon_readable(maps, JOB_ARRAYS & !(PAGE - 1), JOB_ARRAYS + JOB_ARRAY_COUNT * JOB_ARRAY_STRIDE));
        assert!(!maps_private_anon_readable(maps, 0x200000000, 0x200b00000), "a hole");
        assert!(!maps_private_anon_readable(maps, 0x300000000, 0x300010000), "shared");
        assert!(!maps_private_anon_readable(maps, 0x400000000, 0x400010000), "a file");
        assert!(!maps_private_anon_readable(maps, 0x500000000, 0x500010000), "not readable");
        assert!(maps_private_anon_readable(maps, 0x600000000, 0x600010000), "a named anonymous mapping");
        assert!(!maps_private_anon_readable(maps, 0x700000000, 0x700010000), "not mapped");
    }

    #[test]
    fn fresh_heap_blocks_skip_zeroing_and_reused_blocks_are_zeroed() {
        assert!(needs_zeroing(true, false));
        assert!(!needs_zeroing(true, true));
        assert!(!needs_zeroing(false, false));
        assert!(!needs_zeroing(false, true));
        // a size class no other test uses (700 000 + 16 bytes: the 786 432-byte class)
        let size = 700_000;
        unsafe {
            let (p, fresh) = heap_alloc_fresh(size, true);
            assert!(p != 0 && fresh);
            assert!(std::slice::from_raw_parts(p as *const u8, size).iter().all(|&b| b == 0));
            std::ptr::write_bytes(p as *mut u8, 0xAB, size);
            assert!(heap_free(p));
            let (q, fresh) = heap_alloc_fresh(size, true);
            assert_eq!((q, fresh), (p, false), "the freed block comes back from the free list");
            assert!(std::slice::from_raw_parts(q as *const u8, size).iter().all(|&b| b == 0), "a reused block is zeroed");
            std::ptr::write_bytes(q as *mut u8, 0xCD, size);
            assert!(heap_free(q));
            // without the zero flag a reused block keeps its bytes, as before
            let (r, fresh) = heap_alloc_fresh(size, false);
            assert_eq!((r, fresh), (p, false));
            assert_eq!(*(r as *const u8), 0xCD);
            assert!(heap_free(r));
            // a big block is a new mapping
            let (b, fresh) = heap_alloc_fresh(3 << 20, true);
            assert!(b != 0 && fresh);
            assert!(std::slice::from_raw_parts(b as *const u8, 3 << 20).iter().all(|&x| x == 0));
            assert!(heap_free(b));
        }
    }

    #[test]
    fn a_zeroing_realloc_zeroes_the_tail_of_a_reused_block() {
        // a size class no other test uses (600 000 + 16 bytes: the 655 360-byte class)
        let size = 600_000;
        unsafe {
            // a dirty block on the free list
            let d = heap_alloc(size, false);
            std::ptr::write_bytes(d as *mut u8, 0x22, size);
            assert!(heap_free(d));
            let s = heap_alloc(1000, false);
            std::ptr::write_bytes(s as *mut u8, 0x33, 1000);
            let n = heap_realloc(s, size, true, false);
            assert_eq!(n, d, "the realloc reuses the dirty block");
            let v = std::slice::from_raw_parts(n as *const u8, size);
            assert!(v[..1000].iter().all(|&b| b == 0x33));
            assert!(v[1000..].iter().all(|&b| b == 0), "the tail of a reused block is zeroed");
            // the free list of the class is empty now: the next block is fresh and its tail holds zeros unwritten
            let s2 = heap_alloc(1000, false);
            std::ptr::write_bytes(s2 as *mut u8, 0x44, 1000);
            let n2 = heap_realloc(s2, size, true, false);
            assert!(n2 != 0 && n2 != n);
            let v2 = std::slice::from_raw_parts(n2 as *const u8, size);
            assert!(v2[..1000].iter().all(|&b| b == 0x44));
            assert!(v2[1000..].iter().all(|&b| b == 0));
            assert!(heap_free(n));
            assert!(heap_free(n2));
        }
    }

    #[test]
    fn pool_pages_are_fresh_aligned_zero_pages() {
        let a = pool_page();
        let b = pool_page();
        assert!(a != 0 && b != 0 && a != b);
        assert_eq!((a % PAGE, b % PAGE), (0, 0));
        assert!(a < 1 << 44 && b < 1 << 44);
        assert!(unsafe { page_is_zero(a) && page_is_zero(b) });
        assert_eq!(class_of(PAGE + 16).map(|(_, sz)| sz), Some(5120));
    }

    #[test]
    fn a_guarded_slot_ends_its_object_at_the_inaccessible_page() {
        for (size, pages, obj_off) in [(0x3F0, 2, 0xC10), (0x240, 2, 0xDC0), (0x310, 2, 0xCF0), (0x740, 2, 0x8C0), (0x1000, 2, 0), (0x1010, 3, 0xFF0), (0x6000, 7, 0), (0x5D60, 7, 0x2A0)] {
            let s = guard_slot(size);
            assert_eq!((s.pages, s.obj_off), (pages, obj_off), "size {size:#x}");
            assert_eq!(s.obj_off + size, s.guard_off, "the object ends where the guard page starts");
            assert_eq!(s.guard_off % PAGE, 0);
            assert_eq!(s.guard_off + PAGE, s.pages * PAGE, "the guard page is the slot's last");
            assert!(s.obj_off < PAGE, "the fill stays inside the first page");
            assert_eq!(s.obj_off % 16, 0, "16-aligned like a pool slot");
        }
    }

    #[test]
    fn a_guarded_object_is_zero_with_the_fill_before_it_and_no_access_after_it() {
        let size = 0x3F0;
        let a = pool_alloc_guarded(size);
        let b = pool_alloc_guarded(size);
        assert!(a != 0 && b != 0 && a < 1 << 44);
        assert_eq!((a + size) % PAGE, 0);
        assert!(b >= a + size + PAGE, "the next slot starts after the guard page");
        let first = a & !(PAGE - 1);
        let fill = unsafe { std::slice::from_raw_parts(first as *const u8, a - first) };
        assert!(fill.iter().all(|&x| x == GUARD_FILL));
        assert!(unsafe { std::slice::from_raw_parts(a as *const u8, size) }.iter().all(|&x| x == 0));
        let maps = std::fs::read_to_string("/proc/self/maps").unwrap();
        let guard = a + size;
        let line = maps
            .lines()
            .find(|l| {
                let r = l.split_whitespace().next().unwrap();
                let (s, e) = r.split_once('-').unwrap();
                (usize::from_str_radix(s, 16).unwrap()..usize::from_str_radix(e, 16).unwrap()).contains(&guard)
            })
            .unwrap();
        assert!(line.split_whitespace().nth(1).unwrap().starts_with("---"), "{line}");
    }

    #[test]
    fn pool_slots_are_fresh_aligned_zero_and_disjoint() {
        let want = [0x230usize, 0x490, 0x6000, 0x240, 0x3F0, 0x1000, 0x600];
        let mut got: Vec<(usize, usize)> = want.iter().map(|&n| (pool_alloc(n), n)).collect();
        for &(p, n) in &got {
            assert!(p != 0 && p % 16 == 0 && p + n < 1 << 44, "{p:#x} {n:#x}");
            if n >= PAGE {
                assert_eq!(p % PAGE, 0);
            }
            assert!(unsafe { std::slice::from_raw_parts(p as *const u8, n) }.iter().all(|&b| b == 0));
        }
        got.sort();
        for w in got.windows(2) {
            assert!(w[0].0 + w[0].1 <= w[1].0, "{:#x}+{:#x} overlaps {:#x}", w[0].0, w[0].1, w[1].0);
        }
    }
}
