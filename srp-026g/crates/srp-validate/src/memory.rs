//! How much memory a client's game has left (client patch 126,
//! `docs/notes/memory-report.md`): the game is a 32-bit process, carved
//! at its start into fixed arenas and what is left for the video driver,
//! and an arena that is full ends the game. A [`Report`] is a snapshot of
//! each arena the engine registered and of the address space, taken at a
//! [`Moment`] of a match; the server only logs it.
//!
//! The body of a [`crate::report`] carrier of kind memory:
//!
//! ```text
//! u8 moment  u8 count  count x { u8 kind  body[45] }
//!   kind 0 process: u32 free  u32 largest_free  u32 video_low  u32 video_high  zeros[29]
//!   kind 1 arena:   name[32] (printable ASCII, zero-padded)  u32 size  u8 usage  u32 used  u32 largest_free
//!     usage 0 unknown (used and largest_free zero), 1 counted (largest_free zero), 2 heap
//! ```

/// An arena's name's bytes; a longer one is cut.
pub const NAME: usize = 32;
const BODY: usize = 45;
const RECORD: usize = 1 + BODY;
const HEAD: usize = 2;
/// Records in one part: a part's body, with the carrier's header, stays in
/// one 0xE0 chunk ([`crate::report::MAX_SIZE`]).
pub const PER_PART: usize = 10;
/// The arenas the engine's list holds at most (a fixed_vector of 32 in
/// 0.10b's memory.cpp; a full list asserts, register_allocator 0xE21C0).
pub const MAX_ARENAS: usize = 32;
/// Entries of one report: the process, then the arenas.
pub const MAX_ENTRIES: usize = 1 + MAX_ARENAS;
/// Parts of one report.
pub const MAX_PARTS: u8 = MAX_ENTRIES.div_ceil(PER_PART) as u8;
/// A part's body at most.
pub const MAX_BODY: usize = HEAD + PER_PART * RECORD;

/// When in a match the snapshot was taken.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Moment {
    /// The connection's first world is in: the level, its bakes and the
    /// players' textures are loaded.
    Loaded,
    /// The match is closing on the game (close_current_match), its world
    /// still there: the match's own peak.
    Ended,
}

impl Moment {
    pub fn word(self) -> &'static str {
        match self {
            Moment::Loaded => "loaded",
            Moment::Ended => "ended",
        }
    }

    pub(crate) fn byte(self) -> u8 {
        match self {
            Moment::Loaded => 0,
            Moment::Ended => 1,
        }
    }

    fn of(byte: u8) -> Option<Moment> {
        match byte {
            0 => Some(Moment::Loaded),
            1 => Some(Moment::Ended),
            _ => None,
        }
    }
}

/// The process as a whole.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Process {
    /// Free address space in all (`VirtualQuery`'s MEM_FREE regions).
    pub free: u32,
    /// The largest free region: the most one allocation outside the
    /// arenas (the video driver's) can still get.
    pub largest_free: u32,
    /// The video memory budget the engine computed at its start (its
    /// "video memory stats for engine").
    pub video_budget: u64,
}

/// What is known of an arena's bytes in use.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Usage {
    /// Its allocator is of a kind not read, its memory was not all
    /// readable, or its heap changed under the walk (another thread's).
    Unknown,
    /// Bytes taken, as the allocator counts them; it has no free block to
    /// tell (the managed resources' defragment, the multithreaded arena's
    /// pages are taken from a free list).
    Counted { used: u32 },
    /// A dlmalloc heap walked chunk by chunk: the bytes of its chunks in
    /// use (what the engine's "out of memory in arena" dump calls used)
    /// and its largest free block, the never-used top included: the
    /// largest allocation that still fits.
    Heap { used: u32, largest_free: u32 },
}

/// An arena the engine registered.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Arena {
    pub name: Name,
    /// Its bytes, as registered.
    pub size: u32,
    pub usage: Usage,
}

/// An arena's name: 1 to [`NAME`] bytes of printable ASCII.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Name {
    bytes: [u8; NAME],
    len: u8,
}

impl Name {
    /// The name of a C string's bytes (up to its NUL, cut at [`NAME`]);
    /// a byte that is no printable ASCII becomes `?`, no byte at all `?`.
    pub fn of_c(text: &[u8]) -> Name {
        let mut name = Name { bytes: [0; NAME], len: 0 };
        for (slot, &byte) in name.bytes.iter_mut().zip(text.iter().take_while(|&&byte| byte != 0)) {
            *slot = if byte.is_ascii_graphic() || byte == b' ' { byte } else { b'?' };
            name.len = name.len.saturating_add(1);
        }
        if name.len == 0 {
            name.bytes = [0; NAME];
            if let Some(first) = name.bytes.first_mut() {
                *first = b'?';
            }
            name.len = 1;
        }
        name
    }

    pub fn as_str(&self) -> &str {
        self.bytes.get(..usize::from(self.len)).and_then(|bytes| core::str::from_utf8(bytes).ok()).unwrap_or_default()
    }

    fn parse(bytes: &[u8]) -> Option<Name> {
        let bytes: [u8; NAME] = bytes.try_into().ok()?;
        let len = bytes.iter().position(|&byte| byte == 0).unwrap_or(NAME);
        let (text, rest) = bytes.split_at(len);
        if text.is_empty() || !text.iter().all(|&byte| byte.is_ascii_graphic() || byte == b' ') || rest.iter().any(|&byte| byte != 0) {
            return None;
        }
        Some(Name { bytes, len: len as u8 })
    }
}

impl core::fmt::Debug for Name {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(self.as_str(), f)
    }
}

/// One entry of a report.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Entry {
    Process(Process),
    Arena(Arena),
}

fn words(out: &mut [u8], words: &[u32]) {
    for (field, word) in out.chunks_exact_mut(4).zip(words) {
        field.copy_from_slice(&word.to_le_bytes());
    }
}

fn word(bytes: &[u8]) -> Option<u32> {
    bytes.try_into().ok().map(u32::from_le_bytes)
}

impl Entry {
    fn put(&self, record: &mut [u8]) {
        let Some((kind, body)) = record.split_first_mut() else { return };
        match self {
            Entry::Process(process) => {
                *kind = 0;
                words(body, &[process.free, process.largest_free, process.video_budget as u32, (process.video_budget >> 32) as u32]);
            }
            Entry::Arena(arena) => {
                *kind = 1;
                let (name, rest) = body.split_at_mut(NAME);
                name.copy_from_slice(&arena.name.bytes);
                let (size, rest) = rest.split_at_mut(4);
                words(size, &[arena.size]);
                let (usage, rest) = rest.split_at_mut(1);
                let (tag, figures) = match arena.usage {
                    Usage::Unknown => (0, [0, 0]),
                    Usage::Counted { used } => (1, [used, 0]),
                    Usage::Heap { used, largest_free } => (2, [used, largest_free]),
                };
                usage.copy_from_slice(&[tag]);
                words(rest, &figures);
            }
        }
    }

    fn parse(record: &[u8]) -> Option<Entry> {
        let (&kind, body) = record.split_first()?;
        match kind {
            0 => {
                let (figures, zeros) = body.split_at_checked(16)?;
                let mut each = figures.chunks_exact(4).map(word);
                let (Some(Some(free)), Some(Some(largest_free)), Some(Some(low)), Some(Some(high))) = (each.next(), each.next(), each.next(), each.next()) else {
                    return None;
                };
                if zeros.iter().any(|&byte| byte != 0) || largest_free > free {
                    return None;
                }
                Some(Entry::Process(Process { free, largest_free, video_budget: u64::from(high) << 32 | u64::from(low) }))
            }
            1 => {
                let (name, rest) = body.split_at_checked(NAME)?;
                let (size, rest) = rest.split_at_checked(4)?;
                let (&tag, figures) = rest.split_first()?;
                let (used, largest) = figures.split_at_checked(4)?;
                let (size, used, largest) = (word(size)?, word(used)?, word(largest)?);
                let usage = match (tag, used, largest) {
                    (0, 0, 0) => Usage::Unknown,
                    (1, used, 0) => Usage::Counted { used },
                    (2, used, largest_free) => Usage::Heap { used, largest_free },
                    _ => return None,
                };
                Some(Entry::Arena(Arena { name: Name::parse(name)?, size, usage }))
            }
            _ => None,
        }
    }
}

const NOTHING: Entry = Entry::Process(Process { free: 0, largest_free: 0, video_budget: 0 });

/// A whole report, as the client gathers it: the process, then the
/// arenas in the engine's order.
pub struct Report {
    moment: Moment,
    entries: [Entry; MAX_ENTRIES],
    told: usize,
}

impl Report {
    pub fn new(moment: Moment, process: Process) -> Report {
        let mut entries = [NOTHING; MAX_ENTRIES];
        if let Some(first) = entries.first_mut() {
            *first = Entry::Process(process);
        }
        Report { moment, entries, told: 1 }
    }

    pub fn moment(&self) -> Moment {
        self.moment
    }

    /// The next arena; past [`MAX_ARENAS`] (more than the engine's list
    /// holds) it is not kept.
    pub fn add(&mut self, arena: Arena) {
        if let Some(place) = self.entries.get_mut(self.told) {
            *place = Entry::Arena(arena);
            self.told = self.told.saturating_add(1);
        }
    }

    pub fn entries(&self) -> &[Entry] {
        self.entries.get(..self.told).unwrap_or_default()
    }

    /// How many parts it goes in.
    pub fn parts(&self) -> u8 {
        self.told.div_ceil(PER_PART) as u8
    }

    /// The body of part `part` (0-based) into `out`, and its length.
    pub(crate) fn put_part(&self, part: u8, out: &mut [u8]) -> Option<usize> {
        if part >= self.parts() {
            return None;
        }
        let start = usize::from(part).checked_mul(PER_PART)?;
        let entries = self.entries.get(start..self.told.min(start.checked_add(PER_PART)?))?;
        let (head, records) = out.split_at_mut_checked(HEAD)?;
        head.copy_from_slice(&[self.moment.byte(), entries.len() as u8]);
        let mut records = records.chunks_exact_mut(RECORD);
        for entry in entries {
            entry.put(records.next()?);
        }
        HEAD.checked_add(entries.len().checked_mul(RECORD)?)
    }
}

/// The bytes of the body that starts `bytes` (more may follow it), from
/// its count alone: 1 to [`PER_PART`] records after the moment and the
/// count. The rest of it is checked by [`Part::parse`].
pub(crate) fn body_len(bytes: &[u8]) -> Option<usize> {
    let &count = bytes.get(1)?;
    if count == 0 || usize::from(count) > PER_PART {
        return None;
    }
    HEAD.checked_add(usize::from(count).checked_mul(RECORD)?)
}

/// One part as the server reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Part {
    pub moment: Moment,
    entries: [Entry; PER_PART],
    count: u8,
}

impl Part {
    pub fn entries(&self) -> &[Entry] {
        self.entries.get(..usize::from(self.count)).unwrap_or_default()
    }

    /// Exactly a body: a moment, `count` records (full ones only in a part
    /// before the last, `full`), each valid; the process only first in the
    /// report (`first`), arenas after it.
    pub(crate) fn parse(bytes: &[u8], first: bool, full: bool) -> Option<Part> {
        let (head, records) = bytes.split_at_checked(HEAD)?;
        let &[moment, count] = head else { return None };
        let moment = Moment::of(moment)?;
        if count == 0 || usize::from(count) > PER_PART || (full && usize::from(count) != PER_PART) {
            return None;
        }
        if records.len() != usize::from(count).checked_mul(RECORD)? {
            return None;
        }
        let mut entries = [NOTHING; PER_PART];
        for (at, (record, entry)) in records.chunks_exact(RECORD).zip(&mut entries).enumerate() {
            *entry = Entry::parse(record)?;
            if matches!(entry, Entry::Process(_)) != (first && at == 0) {
                return None;
            }
        }
        Some(Part { moment, entries, count })
    }
}

/// The address space walked region by region (`VirtualQuery`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Space {
    free: u32,
    largest_free: u32,
}

impl Space {
    const FREE: u32 = 0x1_0000;

    /// The next region: its `State` and its size.
    pub fn region(&mut self, state: u32, size: u32) {
        if state == Space::FREE {
            self.free = self.free.saturating_add(size);
            self.largest_free = self.largest_free.max(size);
        }
    }

    pub fn process(self, video_budget: u64) -> Process {
        Process { free: self.free, largest_free: self.largest_free, video_budget }
    }
}

/// A region (`VirtualQuery`'s `State` and `Protect`) whose bytes can be
/// read: committed, readable, no guard page.
pub fn readable(state: u32, protect: u32) -> bool {
    const COMMIT: u32 = 0x1000;
    const GUARD: u32 = 0x100;
    state == COMMIT && protect & GUARD == 0 && matches!(protect & 0xFF, 0x02 | 0x04 | 0x08 | 0x20 | 0x40 | 0x80)
}

/// A dlmalloc 2.8 mspace's state (`malloc_state`, client_patch.md section
/// 3.76) as the walk needs it. An arena's mspace is made with its base
/// (`create_mspace_with_base`): one segment, the whole arena, its
/// footprint (and max_footprint, so no high-water mark) the arena's size
/// from the start, its never-used tail the top chunk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mspace {
    pub top: u32,
    pub top_size: u32,
    pub footprint: u32,
    pub segment: u32,
    pub segment_size: u32,
    /// The next segment (0: none).
    pub next_segment: u32,
}

impl Mspace {
    /// Chunks walked at most (1 Mi: a few milliseconds).
    pub const MAX_CHUNKS: u32 = 1 << 20;

    /// Its usage, as the engine's `allocated_size` (internal_mallinfo
    /// 0xFF200, less the mspace's own chunk) counts it, walked from its
    /// segment's first chunk to the top: `head` reads a chunk's head word
    /// (the word at its address + 4; `None` outside the arena). Another
    /// thread may use the heap meanwhile (the game's arenas have no lock):
    /// a walk that does not end at the top, the fence or the segment's end
    /// on a chunk's boundary, or meets a chunk too small to be one, is
    /// `None`. A walk that ends cleanly may still be a little stale: the
    /// state (top, topsize) is read before the walk, and another thread can
    /// move the top or split a chunk meanwhile.
    pub fn usage(self, mut head: impl FnMut(u32) -> Option<u32>) -> Option<Usage> {
        const TOP_FOOT: u32 = 0x28;
        const STATE_CHUNK: u32 = 0x1E0;
        const FENCEPOST: u32 = 7;
        const IN_USE: u32 = 2;
        const MIN_CHUNK: u32 = 16;
        if self.next_segment != 0 {
            return None;
        }
        let end = self.segment.checked_add(self.segment_size)?;
        let mut chunk = self.segment.checked_add(self.segment.wrapping_neg() & 7)?;
        let mut free = self.top_size.checked_add(TOP_FOOT)?;
        let mut largest_free = self.top_size;
        let mut chunks = 0u32;
        while chunk < end && chunk != self.top {
            let word = head(chunk)?;
            if word == FENCEPOST {
                break;
            }
            let size = word & !7;
            if size < MIN_CHUNK {
                return None;
            }
            if word & IN_USE == 0 {
                free = free.checked_add(size)?;
                largest_free = largest_free.max(size);
            }
            chunk = chunk.checked_add(size)?;
            chunks = chunks.checked_add(1).filter(|&n| n <= Mspace::MAX_CHUNKS)?;
        }
        if chunk > end {
            return None;
        }
        let used = self.footprint.checked_sub(free)?.checked_sub(STATE_CHUNK)?;
        Some(Usage::Heap { used, largest_free })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec::Vec;

    fn arena(name: &str, n: u32) -> Arena {
        let usage = match n % 3 {
            0 => Usage::Unknown,
            1 => Usage::Counted { used: n * 1000 },
            _ => Usage::Heap { used: n * 1000, largest_free: n * 7 },
        };
        Arena { name: Name::of_c(name.as_bytes()), size: 0x100_0000 + n, usage }
    }

    fn process() -> Process {
        Process { free: 0x2000_0000, largest_free: 0x0800_0000, video_budget: 0x1_6E2C_D108 }
    }

    #[test]
    fn names_are_printable_ascii_cut_and_never_empty() {
        assert_eq!(Name::of_c(b"render\0junk").as_str(), "render");
        assert_eq!(Name::of_c(b"resources (unmanaged) allocator").as_str(), "resources (unmanaged) allocator");
        assert_eq!(Name::of_c(b"resources (unmanaged) allocator and more").as_str().len(), NAME);
        assert_eq!(Name::of_c(b"a\x07b\xd0").as_str(), "a?b?");
        assert_eq!(Name::of_c(b"").as_str(), "?");
        assert_eq!(Name::of_c(b"\0render").as_str(), "?");
    }

    #[test]
    fn the_address_space_sums_free_regions() {
        let mut space = Space::default();
        for (state, size) in [(0x1000, 0x1000_0000), (0x1_0000, 0x0100_0000), (0x2000, 0x10), (0x1_0000, 0x0300_0000), (0x1_0000, 0x10_0000)] {
            space.region(state, size);
        }
        assert_eq!(space.process(5), Process { free: 0x0410_0000, largest_free: 0x0300_0000, video_budget: 5 });
        assert!(readable(0x1000, 0x04) && readable(0x1000, 0x20) && readable(0x1000, 0x40));
        assert!(!readable(0x1000, 0x01) && !readable(0x1000, 0x104) && !readable(0x2000, 0x04) && !readable(0x1000, 0x10));
    }

    /// An arena as create_mspace_with_base leaves it at `base`, with
    /// chunks of `sizes` (in use when odd in the list's order) and the top.
    fn heap(base: u32, capacity: u32, sizes: &[(u32, bool)]) -> (Mspace, Vec<(u32, u32)>) {
        let mut heads = std::vec![(base, 0x1E3)];
        let mut at = base + 0x1E0;
        for &(size, used) in sizes {
            heads.push((at, size | 1 | if used { 2 } else { 0 }));
            at += size;
        }
        let top_size = base + capacity - at - 0x28;
        heads.push((at, top_size | 1));
        let mspace = Mspace { top: at, top_size, footprint: capacity, segment: base, segment_size: capacity, next_segment: 0 };
        (mspace, heads)
    }

    #[test]
    fn a_heap_walk_counts_as_the_engine_and_finds_the_largest_free_block() {
        let (mspace, heads) = heap(0x1000_0000, 0x10_0000, &[(0x100, true), (0x2_0000, false), (0x40, true), (0x18, false)]);
        let read = |at: u32| heads.iter().find(|&&(chunk, _)| chunk == at).map(|&(_, head)| head);
        // the engine's used: the in-use chunks (the mspace's own chunk not)
        assert_eq!(mspace.usage(read), Some(Usage::Heap { used: 0x140, largest_free: mspace.top_size }));
        let (full, heads) = heap(0x1000_0000, 0x10_0000, &[(0xF_0000, true), (0x8000, false), (0x10, true)]);
        let read = |at: u32| heads.iter().find(|&&(chunk, _)| chunk == at).map(|&(_, head)| head);
        assert_eq!(full.usage(read), Some(Usage::Heap { used: 0xF_0010, largest_free: 0x8000 }));
        // a fresh arena: nothing used, the top all of it
        let (fresh, heads) = heap(0x1000_0000, 0x10_0000, &[]);
        let read = |at: u32| heads.iter().find(|&&(chunk, _)| chunk == at).map(|&(_, head)| head);
        assert_eq!(fresh.usage(read), Some(Usage::Heap { used: 0, largest_free: 0x10_0000 - 0x1E0 - 0x28 }));
    }

    #[test]
    fn a_heap_changed_under_the_walk_is_no_figure() {
        let (mspace, heads) = heap(0x1000_0000, 0x10_0000, &[(0x100, true), (0x2_0000, false), (0x40, true)]);
        let torn = |bad: u32| { let heads = heads.clone(); move |at: u32| heads.iter().find(|&&(chunk, _)| chunk == at).map(|&(chunk, head)| if chunk == 0x1000_0000 + 0x1E0 + 0x100 { bad } else { head }) };
        // a head read as zero (a freed chunk's data), a size past the arena, a size off a boundary
        assert_eq!(mspace.usage(torn(0)), None);
        assert_eq!(mspace.usage(torn(0x8000_0000)), None);
        assert_eq!(mspace.usage(torn(0x2_0008)), None);
        assert_eq!(mspace.usage(|_| None), None);
        assert_eq!(Mspace { next_segment: 1, ..mspace }.usage(|_| Some(3)), None);
        // a walk that never ends at the top is cut
        let endless = Mspace { top: 1, segment_size: 0xFFFF_0000, segment: 0, top_size: 0, footprint: 0xFFFF_0000, next_segment: 0 };
        assert_eq!(endless.usage(|_| Some(16 | 3)), None);
    }

    #[test]
    fn a_report_goes_in_parts_and_back() {
        for arenas in [0, 1, 9, 10, 17, MAX_ARENAS, MAX_ARENAS + 3] {
            let mut report = Report::new(Moment::Ended, process());
            let all: Vec<Arena> = (0..arenas as u32).map(|n| arena(&std::format!("arena {n}"), n)).collect();
            all.iter().for_each(|&a| report.add(a));
            assert!(report.parts() <= MAX_PARTS);
            let mut heard = Vec::new();
            for part in 0..report.parts() {
                let mut out = [0; MAX_BODY];
                let len = report.put_part(part, &mut out).unwrap();
                let full = part + 1 < report.parts();
                let read = Part::parse(&out[..len], part == 0, full).unwrap();
                assert_eq!(read.moment, Moment::Ended);
                heard.extend_from_slice(read.entries());
                assert_eq!(Part::parse(&out[..len], part != 0, full), None, "the process is first, and only there");
                assert_eq!(Part::parse(&out[..len - 1], part == 0, full), None);
            }
            assert_eq!(report.put_part(report.parts(), &mut [0; MAX_BODY]), None);
            let mut told = std::vec![Entry::Process(process())];
            told.extend(all.iter().take(MAX_ARENAS).map(|&a| Entry::Arena(a)));
            assert_eq!(heard, told);
            assert_eq!(report.entries(), &told[..]);
        }
    }

    #[test]
    fn a_body_is_exact() {
        let mut report = Report::new(Moment::Loaded, process());
        (0..3).for_each(|n| report.add(arena("render", n)));
        let mut out = [0; MAX_BODY];
        let len = report.put_part(0, &mut out).unwrap();
        let body = &out[..len];
        let bad = |at: usize, value: u8| {
            let mut bytes = body.to_vec();
            bytes[at] = value;
            Part::parse(&bytes, true, false)
        };
        assert!(Part::parse(body, true, false).is_some());
        assert_eq!(bad(0, 2), None, "a moment");
        assert_eq!(bad(1, 0), None, "no entries");
        assert_eq!(bad(1, 5), None, "a count");
        assert_eq!(Part::parse(body, true, true), None, "a part before the last is full");
        let process_at = HEAD;
        assert_eq!(bad(process_at, 2), None, "a kind");
        assert_eq!(bad(process_at + 1 + 16, 1), None, "the process's zeros");
        assert_eq!(bad(process_at + 1 + 7, 0x30), None, "a largest free region over the free total");
        let unknown_at = HEAD + RECORD;
        let counted_at = HEAD + 2 * RECORD;
        assert_eq!(bad(unknown_at + 1, 0), None, "an empty name");
        assert_eq!(bad(unknown_at + 1 + 3, 0x80), None, "a name that is no ASCII");
        assert_eq!(bad(unknown_at + 1 + 10, b'x'), None, "bytes after the name's end");
        assert_eq!(bad(unknown_at + 1 + NAME + 4, 3), None, "a usage");
        assert_eq!(bad(unknown_at + 1 + NAME + 5, 1), None, "an unknown usage's figures");
        assert_eq!(bad(counted_at + 1 + NAME + 9, 1), None, "a count's largest free block");
    }
}
