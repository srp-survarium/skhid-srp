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

pub unsafe fn heap_alloc(size: usize, zero: bool) -> usize {
    let total = size + 16;
    let (block, cls) = match class_of(total) {
        Some((c, sz)) => {
            let mut g = HEAP.lock().unwrap();
            let h = g.get_or_insert_with(|| Heap { free: vec![Vec::new(); 200], chunk_next: 0, chunk_end: 0 });
            let b = if let Some(b) = h.free[c].pop() {
                b
            } else {
                if h.chunk_next + sz > h.chunk_end {
                    let n = 64 << 20;
                    let a = low_mmap(n, libc::PROT_READ | libc::PROT_WRITE);
                    if a == 0 {
                        return 0;
                    }
                    h.chunk_next = a;
                    h.chunk_end = a + n;
                }
                let b = h.chunk_next;
                h.chunk_next += sz;
                b
            };
            (b, c as u64)
        }
        None => {
            let a = low_mmap(total, libc::PROT_READ | libc::PROT_WRITE);
            if a == 0 {
                return 0;
            }
            (a, BIG)
        }
    };
    unsafe {
        *(block as *mut u64) = size as u64;
        *((block + 8) as *mut u64) = TAG | cls;
        if zero {
            std::ptr::write_bytes((block + 16) as *mut u8, 0, size);
        }
    }
    block + 16
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
    let n = unsafe { heap_alloc(size, false) };
    if n == 0 {
        return 0;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(ptr as *const u8, n as *mut u8, old.min(size));
        if zero && size > old {
            std::ptr::write_bytes((n + old) as *mut u8, 0, size - old);
        }
        heap_free(ptr);
    }
    n
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
