//! HOST_PAGEMAP=1 (diagnosis only): where this process's private pages are, per region, from `/proc/self/pagemap`.
//! A page counts as private when it is present, anonymous and "exclusively mapped" (bit 56: mapped by this process
//! only). A page a fork child still shares with its parent, the shared zero page and a page merged by KSM are not
//! private; a page the child wrote (its copy-on-write copy) or allocated is. Logged as `PAGEMAP [label] ...` lines at a
//! zygote's "zygote ready", in a match child at Ready and at its end, and at the base's cut. It only reads: without the
//! switch nothing is recorded and nothing is read.
//!
//! Bit 56 is reported to every reader of its own pagemap (only the frame numbers need CAP_SYS_ADMIN, since Linux 4.2);
//! every report checks it on a page of its own first and, when the bit does not show, counts every present anonymous
//! page instead (copy-on-write pages still shared with the parent included) and says so.
//!
//! Regions: the exe image (each per-thread job-slot array and its job deque by thread index, the render command pool,
//! the match core buffer, the CRT allocator arena, the rest of .data/BSS in 1 MiB buckets, the rest of the image), the
//! exe's one VirtualAlloc block split by the engine's arena table, other VirtualAlloc regions, the host heap, the
//! null-cook pool by class, the host's low thread stacks, other low mappings, and the host binary, libraries and
//! anonymous memory. Plus the 64 KiB ranges with the most private pages.

use crate::hlog;
use crate::mem::PAGE;
use std::collections::{BTreeMap, HashMap};
use std::os::unix::fs::FileExt;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};

static ON: AtomicBool = AtomicBool::new(false);

/// Reads `HOST_PAGEMAP` once, at process start (the base's zygotes and their match children inherit it).
pub fn init_from_env() {
    ON.store(std::env::var("HOST_PAGEMAP").is_ok_and(|v| v == "1"), Ordering::Relaxed);
}

/// The switch is on.
pub fn on() -> bool {
    ON.load(Ordering::Relaxed)
}

// ---------------------------------------------------------------- records

/// An append-only table of address ranges with a tag, readable without a lock (by a fork child whose parent had
/// another thread in the middle of a push, and by a signal handler). A record is visible once its tag (never 0) is set.
pub struct Records<const N: usize> {
    n: AtomicUsize,
    start: [AtomicUsize; N],
    len: [AtomicUsize; N],
    tag: [AtomicU32; N],
}

impl<const N: usize> Default for Records<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> Records<N> {
    pub const fn new() -> Self {
        Records {
            n: AtomicUsize::new(0),
            start: [const { AtomicUsize::new(0) }; N],
            len: [const { AtomicUsize::new(0) }; N],
            tag: [const { AtomicU32::new(0) }; N],
        }
    }

    /// Append one record (`tag` != 0). False when the table is full (counted in [`Records::dropped`]).
    pub fn push(&self, start: usize, len: usize, tag: u32) -> bool {
        let i = self.n.fetch_add(1, Ordering::Relaxed);
        if i >= N || tag == 0 {
            return false;
        }
        self.start[i].store(start, Ordering::Relaxed);
        self.len[i].store(len, Ordering::Relaxed);
        self.tag[i].store(tag, Ordering::Release);
        true
    }

    /// Records taken so far (some may still be in the middle of their push).
    pub fn count(&self) -> usize {
        self.n.load(Ordering::Acquire).min(N)
    }

    /// Pushes refused because the table was full.
    pub fn dropped(&self) -> usize {
        self.n.load(Ordering::Relaxed).saturating_sub(N)
    }

    /// Record `i` once it is complete: (start, len, tag).
    pub fn get(&self, i: usize) -> Option<(usize, usize, u32)> {
        let t = self.tag.get(i)?.load(Ordering::Acquire);
        (t != 0).then(|| (self.start[i].load(Ordering::Relaxed), self.len[i].load(Ordering::Relaxed), t))
    }

    /// Every complete record.
    pub fn all(&self) -> Vec<(usize, usize, u32)> {
        (0..self.count()).filter_map(|i| self.get(i)).collect()
    }
}

/// Kinds of the host's own low mappings ([`record`]).
pub const HEAP_CHUNK: u32 = 1;
pub const HEAP_BIG: u32 = 2;
pub const POOL_CHUNK: u32 = 3;
pub const LOW_STACK: u32 = 4;

/// The host's low mappings (heap chunks and big blocks, null-cook pool chunks, thread stacks), while the switch is on.
/// A freed big block stays listed: low addresses are never handed out twice, and an unmapped range is not walked.
static RANGES: Records<8192> = Records::new();

/// One of the host's low mappings, recorded while the switch is on (else nothing happens).
pub fn record(kind: u32, start: usize, len: usize) {
    if on() && start != 0 {
        RANGES.push(start, len, kind);
    }
}

// ---------------------------------------------------------------- page entries

/// The flag bits of one `/proc/<pid>/pagemap` entry (Documentation/admin-guide/mm/pagemap.rst).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Entry {
    /// bit 63
    pub present: bool,
    /// bit 62
    pub swapped: bool,
    /// bit 61: a file page or a shared anonymous page
    pub file_or_shared: bool,
    /// bit 56: mapped by this process only
    pub exclusive: bool,
}

pub fn decode(e: u64) -> Entry {
    let bit = |n: u32| (e >> n) & 1 == 1;
    Entry { present: bit(63), swapped: bit(62), file_or_shared: bit(61), exclusive: bit(56) }
}

/// A page this process pays for alone: present, anonymous and (when the kernel shows bit 56) exclusively mapped.
pub fn is_private(e: Entry, bit56: bool) -> bool {
    e.present && !e.file_or_shared && (e.exclusive || !bit56)
}

// ---------------------------------------------------------------- regions

/// The exe image and its writable part (.data/BSS), as `run::mem_breakdown` reads them.
const IMAGE: (usize, usize) = (0x2_0000_0000, 0x2_15E3_8000);
const DATA: (usize, usize) = (0x2_00A9_0000, 0x2_15D9_E000);
/// Each exe thread's job deque, next to its job-slot array (thread init 0x200092179..0x2000921AA: the same index i).
pub const JOB_DEQUES: usize = 0x2_0C08_D340;
pub const JOB_DEQUE_STRIDE: usize = 0x8_0010;
/// The match core buffer (cook 0x20F442720 +0x30 / +0x38).
const CORE: (usize, usize) = (0x2_0F44_2760, 0x2_0F44_2760 + 0x321_D288);
/// The CRT allocator arena inside .data (the engine's start-up line "memory stats for arena [CRT allocator]").
const CRT_ARENA: (usize, usize) = (0x2_00CF_2CA0, 0x2_01CF_2CA0);

/// The exe's fixed arenas in its one VirtualAlloc block (3079 MB), as offsets from the block's start: the engine's own
/// start-up lines "memory stats for arena [...]" give each arena's start and end address (block at 0x10014090000
/// there), in this order.
pub const ARENAS: [(&str, usize, usize); 16] = [
    ("shared strings", 0x0, 0x10_0000),
    ("global multithreaded", 0x10_0000, 0x3990_0000),
    ("cook allocator", 0x3990_0000, 0x39A2_0000),
    ("resources helper allocator", 0x39A2_0000, 0x4222_0000),
    ("resources links allocator", 0x4222_0000, 0x4229_FFF8),
    ("engine", 0x4229_FFF8, 0x422D_FFF8),
    ("render", 0x422D_FFF8, 0x42AD_FFF8),
    ("network", 0x42AD_FFF8, 0x45AD_FFF8),
    ("sound", 0x45AD_FFF8, 0x46ED_FFF8),
    ("physics", 0x46ED_FFF8, 0x546D_FFF8),
    ("input", 0x546D_FFF8, 0x546E_FFF8),
    ("ai navigation", 0x546E_FFF8, 0x546F_FFF8),
    ("ai", 0x546F_FFF8, 0x548F_FFF8),
    ("survarium", 0x548F_FFF8, 0x606F_FFF8),
    ("resources (managed) allocator", 0x606F_FFF8, 0xA86F_FFF8),
    ("resources (unmanaged) allocator", 0xA86F_FFF8, 0xC06F_FFF8),
];

/// Where a page is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Region {
    JobArray(u8),
    JobDeque(u8),
    RenderPool,
    CoreBuffer,
    CrtArena,
    /// .data/BSS outside the regions above: 1 MiB bucket i from the start of .data
    Data(u16),
    /// the image outside .data/BSS (headers, code, read-only data)
    ImageOther,
    Arena(u8),
    ArenaRest,
    VirtualAllocOther,
    HostHeap,
    NullPool(u32),
    /// pool pages no stand-in touches (the chunk's unused end, the pages after guarded stand-ins)
    NullPoolFree,
    LowStack,
    LowOther,
    HostBinary,
    HostLibs,
    HostAnon,
}

impl Region {
    pub fn label(self) -> String {
        match self {
            Region::JobArray(i) => format!("job-slot array {i}"),
            Region::JobDeque(i) => format!("job deque {i}"),
            Region::RenderPool => "render command pool".into(),
            Region::CoreBuffer => format!("match core buffer {:#x}+{:#x}", CORE.0, CORE.1 - CORE.0),
            Region::CrtArena => "CRT allocator arena (.data)".into(),
            Region::Data(i) => format!(".data/BSS {:#x}", DATA.0 + (usize::from(i) << 20)),
            Region::ImageOther => "image headers, code and read-only data".into(),
            Region::Arena(i) => format!("exe arena {}", ARENAS[usize::from(i)].0),
            Region::ArenaRest => "exe VirtualAlloc block after the arena table".into(),
            Region::VirtualAllocOther => "other VirtualAlloc regions".into(),
            Region::HostHeap => "host heap (chunks and big blocks)".into(),
            Region::NullPool(c) => format!("null-cook pool class {c:#x}"),
            Region::NullPoolFree => "null-cook pool pages without a stand-in".into(),
            Region::LowStack => "host low thread stacks".into(),
            Region::LowOther => "other low mappings (thread blocks, import thunks)".into(),
            Region::HostBinary => "host binary".into(),
            Region::HostLibs => "host libraries".into(),
            Region::HostAnon => "host anonymous memory (Rust heap, stacks)".into(),
        }
    }
}

/// The image region of `a`, or None outside the image.
pub fn image_region(a: usize) -> Option<Region> {
    if !(IMAGE.0..IMAGE.1).contains(&a) {
        return None;
    }
    let jobs = crate::mem::JOB_ARRAYS;
    let (count, stride) = (crate::mem::JOB_ARRAY_COUNT, crate::mem::JOB_ARRAY_STRIDE);
    if (jobs..jobs + count * stride).contains(&a) {
        return Some(Region::JobArray(((a - jobs) / stride) as u8));
    }
    if (JOB_DEQUES..JOB_DEQUES + count * JOB_DEQUE_STRIDE).contains(&a) {
        return Some(Region::JobDeque(((a - JOB_DEQUES) / JOB_DEQUE_STRIDE) as u8));
    }
    let pool = (crate::world::RENDER_POOL as usize, crate::world::RENDER_POOL_END as usize);
    if (pool.0..pool.1).contains(&a) {
        return Some(Region::RenderPool);
    }
    if (CORE.0..CORE.1).contains(&a) {
        return Some(Region::CoreBuffer);
    }
    if (CRT_ARENA.0..CRT_ARENA.1).contains(&a) {
        return Some(Region::CrtArena);
    }
    if (DATA.0..DATA.1).contains(&a) {
        return Some(Region::Data(((a - DATA.0) >> 20) as u16));
    }
    Some(Region::ImageOther)
}

/// The arena (index into [`ARENAS`]) of offset `off` into the VirtualAlloc block.
pub fn arena_at(off: usize) -> Option<usize> {
    ARENAS.iter().position(|&(_, s, e)| (s..e).contains(&off))
}

/// The region of the span holding `a` (`spans` sorted by start, not overlapping).
fn span_at(spans: &[(usize, usize, Region)], a: usize) -> Option<Region> {
    let i = spans.partition_point(|s| s.0 <= a);
    let s = spans.get(i.checked_sub(1)?)?;
    (a < s.1).then_some(s.2)
}

/// The class of every page a null-cook stand-in touches: (start, size, class) records; a page that holds stand-ins of
/// two classes goes to the one at the lower address.
pub fn page_classes(standins: &[(usize, usize, u32)]) -> HashMap<usize, u32> {
    let mut v: Vec<&(usize, usize, u32)> = standins.iter().filter(|s| s.1 != 0).collect();
    v.sort_by_key(|s| s.0);
    let mut m = HashMap::new();
    for &&(a, n, c) in &v {
        for p in a / PAGE..=(a + n - 1) / PAGE {
            m.entry(p).or_insert(c);
        }
    }
    m
}

/// One line of /proc/self/maps.
#[derive(Debug, PartialEq, Eq)]
pub struct Mapping {
    pub start: usize,
    pub end: usize,
    pub readable: bool,
    pub writable: bool,
    /// backed by a file (inode != 0)
    pub file: bool,
    pub name: String,
}

pub fn parse_maps_line(l: &str) -> Option<Mapping> {
    let mut it = l.split_whitespace();
    let (range, perm, _off, _dev, inode) = (it.next()?, it.next()?, it.next()?, it.next()?, it.next()?);
    let (a, b) = range.split_once('-')?;
    let p = perm.as_bytes();
    Some(Mapping {
        start: usize::from_str_radix(a, 16).ok()?,
        end: usize::from_str_radix(b, 16).ok()?,
        readable: p.first() == Some(&b'r'),
        writable: p.get(1) == Some(&b'w'),
        file: inode != "0",
        name: it.collect::<Vec<_>>().join(" "),
    })
}

/// Mappings a walk leaves out: no access (reserved ranges, guard pages) and the kernel's own pages.
fn skipped(m: &Mapping) -> bool {
    (!m.readable && !m.writable) || matches!(m.name.as_str(), "[vsyscall]" | "[vvar]" | "[vvar_vclock]" | "[vdso]")
}

// ---------------------------------------------------------------- the walk

/// Entries read per pagemap read (512 KiB).
const CHUNK: usize = 1 << 16;

/// Whether bit 56 shows on a page this process just wrote (it must: the page is mapped by this process only).
fn bit56_works(f: &std::fs::File) -> bool {
    unsafe {
        let p = libc::mmap(std::ptr::null_mut(), PAGE, libc::PROT_READ | libc::PROT_WRITE, libc::MAP_PRIVATE | libc::MAP_ANONYMOUS, -1, 0);
        if p == libc::MAP_FAILED {
            return false;
        }
        *(p as *mut u8) = 1;
        let mut b = [0u8; 8];
        let ok = f.read_exact_at(&mut b, (p as usize / PAGE * 8) as u64).is_ok();
        libc::munmap(p, PAGE);
        let e = decode(u64::from_le_bytes(b));
        ok && e.present && e.exclusive
    }
}

#[derive(Default)]
struct Walk {
    bit56: bool,
    mappings: usize,
    unreadable: usize,
    swapped: u64,
    /// per region: (present pages, private pages)
    sums: BTreeMap<Region, (u64, u64)>,
    /// private pages per 64 KiB range, with the region of its first private page
    ranges: HashMap<usize, (u64, Region)>,
    /// stand-ins per class
    standins: BTreeMap<u32, usize>,
    block: Option<usize>,
    ms: u128,
}

fn walk() -> Result<Walk, String> {
    let t0 = std::time::Instant::now();
    let f = std::fs::File::open("/proc/self/pagemap").map_err(|e| format!("/proc/self/pagemap: {e}"))?;
    let maps = std::fs::read_to_string("/proc/self/maps").map_err(|e| format!("/proc/self/maps: {e}"))?;
    let mut w = Walk { bit56: bit56_works(&f), ..Default::default() };
    // the low spans: the VirtualAlloc regions (the largest one, at least 1 GiB, is the arena block) and the host's own
    let mut spans: Vec<(usize, usize, Region)> = Vec::new();
    let regions = crate::mem::virtual_alloc_regions();
    w.block = regions.iter().copied().filter(|r| r.1 - r.0 >= 1 << 30).max_by_key(|r| r.1 - r.0).map(|r| r.0);
    for &(a, b) in &regions {
        if Some(a) == w.block {
            for (i, &(_, s, e)) in ARENAS.iter().enumerate() {
                if a + s < b {
                    spans.push((a + s, (a + e).min(b), Region::Arena(i as u8)));
                }
            }
            let last = a + ARENAS[ARENAS.len() - 1].2;
            if last < b {
                spans.push((last, b, Region::ArenaRest));
            }
        } else {
            spans.push((a, b, Region::VirtualAllocOther));
        }
    }
    for (s, n, k) in RANGES.all() {
        let r = match k {
            HEAP_CHUNK | HEAP_BIG => Region::HostHeap,
            POOL_CHUNK => Region::NullPoolFree,
            LOW_STACK => Region::LowStack,
            _ => Region::LowOther,
        };
        spans.push((s, s + n, r));
    }
    spans.sort_by_key(|s| s.0);
    let standins = crate::nullcook::standins();
    for s in &standins {
        *w.standins.entry(s.2).or_default() += 1;
    }
    let classes = page_classes(&standins);
    let mut buf = vec![0u8; CHUNK * 8];
    for m in maps.lines().filter_map(parse_maps_line) {
        if skipped(&m) {
            continue;
        }
        w.mappings += 1;
        let low = (crate::mem::LOW_START..crate::mem::LOW_END).contains(&m.start);
        let fallback = if low {
            Region::LowOther
        } else if m.name.contains("world-host") {
            Region::HostBinary
        } else if m.file {
            Region::HostLibs
        } else {
            Region::HostAnon
        };
        let mut a = m.start;
        while a < m.end {
            let n = ((m.end - a) / PAGE).min(CHUNK);
            if f.read_exact_at(&mut buf[..n * 8], (a / PAGE * 8) as u64).is_err() {
                w.unreadable += 1;
                break;
            }
            for i in 0..n {
                let e = decode(u64::from_le_bytes(buf[i * 8..i * 8 + 8].try_into().unwrap()));
                if e.swapped {
                    w.swapped += 1;
                }
                if !e.present {
                    continue;
                }
                let addr = a + i * PAGE;
                let mut r = image_region(addr).or_else(|| span_at(&spans, addr)).unwrap_or(fallback);
                if r == Region::NullPoolFree
                    && let Some(&c) = classes.get(&(addr / PAGE))
                {
                    r = Region::NullPool(c);
                }
                let s = w.sums.entry(r).or_default();
                s.0 += 1;
                if is_private(e, w.bit56) {
                    s.1 += 1;
                    w.ranges.entry(addr >> 16).or_insert((0, r)).0 += 1;
                }
            }
            a += n * PAGE;
        }
    }
    w.ms = t0.elapsed().as_millis();
    Ok(w)
}

fn kb(pages: u64) -> u64 {
    pages * (PAGE as u64 / 1024)
}

/// "i:kB" for every region of one kind with private pages, and the kind's total.
fn group(w: &Walk, pick: impl Fn(Region) -> Option<String>) -> Option<String> {
    let mut parts = Vec::new();
    let (mut present, mut private) = (0, 0);
    for (r, &(p, x)) in &w.sums {
        if let Some(k) = pick(*r) {
            present += p;
            private += x;
            if x > 0 {
                parts.push(format!("{k}:{}", kb(x)));
            }
        }
    }
    (present > 0).then(|| format!("{}; total {} kB private of {} kB present", if parts.is_empty() { "none".into() } else { parts.join(" ") }, kb(private), kb(present)))
}

/// The `PAGEMAP [label] ...` lines (nothing without HOST_PAGEMAP=1).
pub fn report(label: &str) {
    if !on() {
        return;
    }
    let w = match walk() {
        Ok(w) => w,
        Err(e) => {
            hlog!("PAGEMAP [{label}] not read: {e}");
            return;
        }
    };
    let (present, private): (u64, u64) = w.sums.values().fold((0, 0), |a, v| (a.0 + v.0, a.1 + v.1));
    hlog!(
        "PAGEMAP [{label}] {}: private {} kB, present {} kB, swapped {} kB; {} mappings read ({} not readable) in {} ms",
        if w.bit56 {
            "private = present, anonymous and exclusively mapped (bit 56, seen on a page of this process)"
        } else {
            "bit 56 NOT seen on a page of this process: private = every present anonymous page (copy-on-write pages still shared included)"
        },
        kb(private),
        kb(present),
        kb(w.swapped),
        w.mappings,
        w.unreadable,
        w.ms
    );
    if let Some(s) = group(&w, |r| if let Region::JobArray(i) = r { Some(i.to_string()) } else { None }) {
        hlog!("PAGEMAP [{label}] job-slot arrays (private kB by thread index): {s}");
    }
    if let Some(s) = group(&w, |r| if let Region::JobDeque(i) = r { Some(i.to_string()) } else { None }) {
        hlog!("PAGEMAP [{label}] job deques (private kB by thread index): {s}");
    }
    for r in [Region::RenderPool, Region::CoreBuffer, Region::CrtArena, Region::ImageOther] {
        if let Some(&(p, x)) = w.sums.get(&r) {
            hlog!("PAGEMAP [{label}] {}: private {} kB, present {} kB", r.label(), kb(x), kb(p));
        }
    }
    if let Some(s) = group(&w, |r| if let Region::Data(i) = r { Some(format!("{:#x}", DATA.0 + (usize::from(i) << 20))) } else { None }) {
        hlog!("PAGEMAP [{label}] rest of .data/BSS (private kB by 1 MiB bucket): {s}");
    }
    let n_standins: usize = w.standins.values().sum();
    if let Some(s) = group(&w, |r| match r {
        Region::NullPool(c) => Some(format!("{c:#x}({} stand-ins)", w.standins.get(&c).copied().unwrap_or(0))),
        Region::NullPoolFree => Some("no-stand-in".into()),
        _ => None,
    }) {
        hlog!("PAGEMAP [{label}] null-cook pool by class (private kB): {s}; {n_standins} stand-ins recorded");
    }
    if let Some(s) = group(&w, |r| match r {
        Region::Arena(i) => Some(ARENAS[usize::from(i)].0.replace(' ', "_")),
        Region::ArenaRest => Some("after_the_table".into()),
        _ => None,
    }) {
        hlog!("PAGEMAP [{label}] exe VirtualAlloc block {:#x} by the engine's arena table (private kB): {s}", w.block.unwrap_or(0));
    }
    for r in [Region::VirtualAllocOther, Region::HostHeap, Region::LowStack, Region::LowOther, Region::HostBinary, Region::HostLibs, Region::HostAnon] {
        if let Some(&(p, x)) = w.sums.get(&r) {
            hlog!("PAGEMAP [{label}] {}: private {} kB, present {} kB", r.label(), kb(x), kb(p));
        }
    }
    let mut top: Vec<(&usize, &(u64, Region))> = w.ranges.iter().collect();
    top.sort_by(|a, b| b.1.0.cmp(&a.1.0).then(a.0.cmp(b.0)));
    let list: Vec<String> = top.iter().take(24).map(|(k, (n, r))| format!("{:#x}:{}({})", *k << 16, kb(*n), r.label())).collect();
    hlog!("PAGEMAP [{label}] top 64 KiB ranges (private kB): {}", list.join(" "));
    let dropped = (RANGES.dropped(), crate::nullcook::standins_dropped());
    if dropped != (0, 0) {
        hlog!("PAGEMAP [{label}] records full: {} low mappings and {} stand-ins not recorded (their pages count as other)", dropped.0, dropped.1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_entry_decodes_its_flag_bits() {
        let e = decode(1 << 63 | 1 << 56 | 0x1234);
        assert_eq!(e, Entry { present: true, swapped: false, file_or_shared: false, exclusive: true });
        assert!(is_private(e, true));
        let shared = decode(1 << 63);
        assert!(!is_private(shared, true), "present, not exclusive: still shared with the parent");
        assert!(is_private(shared, false), "without bit 56 every present anonymous page counts");
        let file = decode(1 << 63 | 1 << 61 | 1 << 56);
        assert!(file.file_or_shared && !is_private(file, true) && !is_private(file, false));
        let swapped = decode(1 << 62 | 1 << 56);
        assert!(swapped.swapped && !swapped.present && !is_private(swapped, false));
        assert_eq!(decode(0), Entry::default());
    }

    #[test]
    fn image_pages_fall_into_their_regions() {
        let (jobs, stride) = (crate::mem::JOB_ARRAYS, crate::mem::JOB_ARRAY_STRIDE);
        assert_eq!(image_region(jobs), Some(Region::JobArray(0)));
        assert_eq!(image_region(jobs + stride - 1), Some(Region::JobArray(0)));
        assert_eq!(image_region(jobs + stride), Some(Region::JobArray(1)));
        assert_eq!(image_region(jobs + 27 * stride - 1), Some(Region::JobArray(26)));
        assert_eq!(image_region(jobs + 27 * stride), Some(Region::Data(((jobs + 27 * stride - DATA.0) >> 20) as u16)));
        assert_eq!(image_region(JOB_DEQUES), Some(Region::JobDeque(0)));
        assert_eq!(image_region(JOB_DEQUES + 3 * JOB_DEQUE_STRIDE + 8), Some(Region::JobDeque(3)));
        assert_eq!(image_region(crate::world::RENDER_POOL as usize), Some(Region::RenderPool));
        assert_eq!(image_region(crate::world::RENDER_POOL_END as usize - 1), Some(Region::RenderPool));
        assert_eq!(image_region(0x2_0F44_2760), Some(Region::CoreBuffer));
        assert_eq!(image_region(0x2_0F44_2760 + 0x321_D287), Some(Region::CoreBuffer));
        assert_eq!(image_region(0x2_00CF_2CA0), Some(Region::CrtArena));
        assert_eq!(image_region(DATA.0), Some(Region::Data(0)));
        assert_eq!(image_region(DATA.0 + (5 << 20) + 7), Some(Region::CrtArena), "the CRT arena lies inside bucket 2..18");
        assert_eq!(image_region(DATA.0 + (32 << 20) + 7), Some(Region::Data(32)));
        assert_eq!(image_region(0x2_0000_1000), Some(Region::ImageOther));
        assert_eq!(image_region(0x1_FFFF_F000), None);
        assert_eq!(image_region(IMAGE.1), None);
        // the named regions do not overlap one another
        let named = [
            (jobs, jobs + 27 * stride),
            (JOB_DEQUES, JOB_DEQUES + 27 * JOB_DEQUE_STRIDE),
            (crate::world::RENDER_POOL as usize, crate::world::RENDER_POOL_END as usize),
            CORE,
            CRT_ARENA,
        ];
        for (i, a) in named.iter().enumerate() {
            assert!(a.0 >= DATA.0 && a.1 <= DATA.1, "{a:x?} inside .data/BSS");
            for b in &named[i + 1..] {
                assert!(a.1 <= b.0 || b.1 <= a.0, "{a:x?} and {b:x?} overlap");
            }
        }
    }

    #[test]
    fn the_arena_table_gives_the_engines_own_addresses() {
        // the engine's start-up lines of a recorded zygote: block 0x10014090000, each arena's start address
        let block = 0x100_1409_0000usize;
        let logged = [
            0x100_1409_0000usize,
            0x100_1419_0000,
            0x100_4d99_0000,
            0x100_4dab_0000,
            0x100_562b_0000,
            0x100_5632_fff8,
            0x100_5636_fff8,
            0x100_56b6_fff8,
            0x100_59b6_fff8,
            0x100_5af6_fff8,
            0x100_6876_fff8,
            0x100_6877_fff8,
            0x100_6878_fff8,
            0x100_6898_fff8,
            0x100_7478_fff8,
            0x100_bc78_fff8,
        ];
        for (i, &a) in logged.iter().enumerate() {
            assert_eq!(block + ARENAS[i].1, a, "{}", ARENAS[i].0);
            assert_eq!(arena_at(a - block), Some(i));
        }
        // contiguous, ending at the unmanaged arena's logged end, inside the 3079 MB block
        for w in ARENAS.windows(2) {
            assert_eq!(w[0].2, w[1].1);
        }
        assert_eq!(block + ARENAS[15].2, 0x100_d478_fff8);
        assert!(ARENAS[15].2 <= 3079 << 20);
        assert_eq!(arena_at(ARENAS[15].2), None);
        // the sizes the engine prints ("size: N")
        assert_eq!(ARENAS[1].2 - ARENAS[1].1, 964_689_920);
        assert_eq!(ARENAS[4].2 - ARENAS[4].1, 524_280);
        assert_eq!(ARENAS[9].2 - ARENAS[9].1, 226_492_416);
        assert_eq!(ARENAS[14].2 - ARENAS[14].1, 1_207_959_552);
    }

    #[test]
    fn pages_of_the_null_pool_take_the_class_of_their_first_stand_in() {
        let p = 0x100_0000_0000usize;
        let s = [(p + 0x3F0, 0x600, 0x3D), (p, 0x3F0, 0x18), (p + 0x1000, 0x1000, 0x13), (p + 0x2000, 0x6000, 0x14), (p + 0x9000, 0, 0x6)];
        let m = page_classes(&s);
        assert_eq!(m.get(&(p / PAGE)), Some(&0x18), "the page starts with the 0x18 stand-in");
        assert_eq!(m.get(&(p / PAGE + 1)), Some(&0x13));
        for i in 2..8 {
            assert_eq!(m.get(&(p / PAGE + i)), Some(&0x14), "page {i} of the 0x14 slot");
        }
        assert_eq!(m.get(&(p / PAGE + 8)), None);
        assert_eq!(m.get(&(p / PAGE + 9)), None, "an empty record touches no page");
        assert_eq!(m.len(), 8);
    }

    #[test]
    fn a_span_is_found_by_its_start_and_end() {
        let spans = [(0x1000, 0x3000, Region::HostHeap), (0x3000, 0x4000, Region::LowStack), (0x8000, 0x9000, Region::NullPoolFree)];
        assert_eq!(span_at(&spans, 0xFFF), None);
        assert_eq!(span_at(&spans, 0x1000), Some(Region::HostHeap));
        assert_eq!(span_at(&spans, 0x2FFF), Some(Region::HostHeap));
        assert_eq!(span_at(&spans, 0x3000), Some(Region::LowStack));
        assert_eq!(span_at(&spans, 0x4000), None);
        assert_eq!(span_at(&spans, 0x8800), Some(Region::NullPoolFree));
        assert_eq!(span_at(&spans, 0x9000), None);
        assert_eq!(span_at(&[], 0x9000), None);
    }

    #[test]
    fn maps_lines_parse_and_inaccessible_mappings_are_left_out() {
        let m = parse_maps_line("200a90000-215d9e000 rw-p 00000000 00:00 0 ").unwrap();
        assert_eq!((m.start, m.end, m.readable, m.writable, m.file, m.name.as_str()), (0x200a90000, 0x215d9e000, true, true, false, ""));
        assert!(!skipped(&m));
        let f = parse_maps_line("7f0000000000-7f0000001000 r--p 00001000 08:01 777 /usr/lib/x86_64-linux-gnu/libc.so.6").unwrap();
        assert!(f.file && f.readable && !f.writable && f.name.ends_with("libc.so.6"));
        let sp = parse_maps_line("7f0000000000-7f0000001000 rw-p 00000000 08:01 777 /home/me/a b").unwrap();
        assert_eq!(sp.name, "/home/me/a b");
        assert!(skipped(&parse_maps_line("100140b0000-100d4790000 ---p 00000000 00:00 0").unwrap()));
        assert!(skipped(&parse_maps_line("7ffd0000-7ffd2000 r--p 00000000 00:00 0 [vvar]").unwrap()));
        assert!(!skipped(&parse_maps_line("7ffd0000-7ffd2000 rw-p 00000000 00:00 0 [stack]").unwrap()));
        assert_eq!(parse_maps_line("garbage"), None);
    }

    #[test]
    fn records_take_pushes_up_to_their_size() {
        let r: Records<3> = Records::new();
        assert!(r.push(0x1000, 0x10, 7));
        assert!(!r.push(0x2000, 0x10, 0), "tag 0 is refused");
        assert!(r.push(0x3000, 0x20, 9));
        assert!(!r.push(0x4000, 0x20, 9), "full");
        assert_eq!(r.all(), vec![(0x1000, 0x10, 7), (0x3000, 0x20, 9)]);
        assert_eq!(r.dropped(), 1);
        assert_eq!(r.get(1), None, "the refused tag-0 slot stays empty");
    }

    #[test]
    fn bit_56_marks_a_written_page_and_not_one_shared_with_a_fork_child() {
        let f = std::fs::File::open("/proc/self/pagemap").unwrap();
        assert!(bit56_works(&f), "bit 56 is not shown to this (unprivileged) reader");
        unsafe {
            let a = libc::mmap(std::ptr::null_mut(), PAGE, libc::PROT_READ | libc::PROT_WRITE, libc::MAP_PRIVATE | libc::MAP_ANONYMOUS, -1, 0);
            assert_ne!(a, libc::MAP_FAILED);
            *(a as *mut u8) = 1;
            let off = (a as usize / PAGE * 8) as i64;
            let pid = libc::fork();
            if pid == 0 {
                // the child: only system calls (no allocation after fork in a threaded test process)
                let fd = libc::open(c"/proc/self/pagemap".as_ptr(), libc::O_RDONLY);
                let mut v = 0u64;
                let read = |v: &mut u64| libc::pread(fd, v as *mut u64 as *mut libc::c_void, 8, off) == 8;
                let mut code = 0;
                if !read(&mut v) || v >> 63 & 1 != 1 || v >> 56 & 1 != 0 {
                    code |= 1; // shared with the parent: present, not exclusive
                }
                *(a as *mut u8) = 2;
                if !read(&mut v) || v >> 63 & 1 != 1 || v >> 56 & 1 != 1 {
                    code |= 2; // its own copy after the write: exclusive
                }
                libc::_exit(code);
            }
            assert!(pid > 0);
            let mut st = 0;
            assert_eq!(libc::waitpid(pid, &mut st, 0), pid);
            assert!(libc::WIFEXITED(st), "status {st:#x}");
            assert_eq!(libc::WEXITSTATUS(st), 0, "1: a shared page showed as exclusive; 2: the child's copy did not");
            libc::munmap(a, PAGE);
        }
    }
}
