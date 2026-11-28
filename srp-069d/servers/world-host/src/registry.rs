//! The exe's resource name cache, read by the host and never written: the late-load trace of a match child (what a
//! match registers in the cache after its creation query) and the cache's size in a zygote.
//!
//! The cache (from the exe's find 0x2000C2A10, insert 0x2000CB360 and erase 0x2000A8250): one hash table of 0x8000
//! buckets at 0x200C71270, the first node of bucket i at +8 + i * 8, and the entry count (u32) right after the last
//! bucket, at 0x200CB1278. A node holds the resource class (u32) at +0, its owner at +8, a pointer to the resource's
//! zero-terminated name at +0x10 and the next node of its bucket at +0x18. Two places insert: the lookup 0x2000CAF70
//! when a name is not in the table (0x2000CB019: the request's own node, before it is cooked) and the completion path
//! (0x2000CAC2D: the finished resource's node, with a copy of its name, in place of the request's node); every insert
//! adds 1 to the count, every erase takes 1 off. The exe takes the critical section 0x200CB1280 around every find,
//! insert and erase (taken at 0x2000A7A90, 0x2000C1EA0, 0x2000CABC0, 0x2000CAFC0). Nothing in the exe walks the table
//! or reads the count: the table's address occurs only in those finds, inserts and erases, and the count is only
//! changed by insert and erase.
//!
//! So the host reads the count after a step without the lock (one aligned u32; a changed value only starts a walk) and
//! walks the table only under the exe's own lock, taken and released with the host's TryEnterCriticalSection /
//! LeaveCriticalSection (the calls an exe thread makes; the section's bytes are those of a free section again after
//! it). An exe thread that wants the lock meanwhile waits, which costs it time only. No exe byte is written.

use crate::hlog;
use std::collections::HashSet;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// The name cache's hash table.
pub const TABLE: u64 = 0x200C71270;
/// Its buckets.
pub const BUCKETS: u64 = 0x8000;
/// Its entry count (u32), right after the last bucket.
pub const COUNT: u64 = TABLE + 8 + BUCKETS * 8;
/// The critical section the exe holds around every find, insert and erase of the table.
pub const LOCK: u64 = 0x200CB1280;
/// A node's name pointer and next node.
const NODE_NAME: u64 = 0x10;
const NODE_NEXT: u64 = 0x18;
/// The longest name read (a longer one is cut there; the exe's resource names are paths of a few hundred bytes).
pub const MAX_NAME: usize = 4096;
/// The most nodes one walk visits: a damaged chain cannot hang it (a cache holds some thousands).
pub const MAX_NODES: usize = 1 << 20;
/// How long a walk waits for the lock; then it is skipped (and counted).
const LOCK_WAIT: Duration = Duration::from_millis(50);
/// The image's address range and the host's low area: where the exe's nodes and names live.
const IMAGE: std::ops::Range<u64> = 0x2_0000_0000..0x3_0000_0000;

/// The entry count, read without the lock.
pub fn count() -> u32 {
    // SAFETY: an aligned u32 in the exe's writable image, which is mapped for the whole run
    unsafe { std::ptr::read_volatile(COUNT as *const u32) }
}

/// A node or name pointer the walk may read: in the host's low area or the image.
fn exe_ptr(p: u64) -> bool {
    ((crate::mem::LOW_START as u64)..(crate::mem::LOW_END as u64)).contains(&p) || IMAGE.contains(&p)
}

/// What one walk saw: nodes visited, nodes or names it could not read (skipped), whether it stopped at its node
/// limit, and the entry count read with the table.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Walk {
    pub nodes: usize,
    pub bad: usize,
    pub cut: bool,
    pub count: u32,
}

/// Every node of the table at `table` (`buckets` bucket heads at +8, the count after them), bucket by bucket and chain
/// by chain: `f(class, name bytes)`. A node or name pointer that `ok` refuses (or a node not 8-aligned) is not read: a
/// bad node ends its chain, a bad name skips its node; both are counted. At most `max_nodes` nodes.
///
/// # Safety
/// The bucket heads and the count must be readable, and every pointer `ok` accepts must point at a readable node
/// (0x20 bytes) or a zero-terminated name (or at least MAX_NAME readable bytes).
pub unsafe fn walk_at(table: u64, buckets: u64, max_nodes: usize, ok: impl Fn(u64) -> bool, mut f: impl FnMut(u32, &[u8])) -> Walk {
    unsafe { walk_at_nodes(table, buckets, max_nodes, ok, |_, c, n| f(c, n)) }
}

/// [`walk_at`] that also gives each node's address: `f(node, class, name bytes)`.
///
/// # Safety
/// As [`walk_at`].
pub unsafe fn walk_at_nodes(table: u64, buckets: u64, max_nodes: usize, ok: impl Fn(u64) -> bool, mut f: impl FnMut(u64, u32, &[u8])) -> Walk {
    let mut w = Walk::default();
    let mut name = Vec::with_capacity(256);
    unsafe {
        w.count = std::ptr::read_volatile((table + 8 + buckets * 8) as *const u32);
        for i in 0..buckets {
            let mut n = *((table + 8 + i * 8) as *const u64);
            while n != 0 {
                if w.nodes >= max_nodes {
                    w.cut = true;
                    return w;
                }
                if !n.is_multiple_of(8) || !ok(n) {
                    w.bad += 1;
                    break;
                }
                w.nodes += 1;
                let class = *(n as *const u32);
                let p = *((n + NODE_NAME) as *const u64);
                if p != 0 && ok(p) {
                    name.clear();
                    for k in 0..MAX_NAME as u64 {
                        let c = *((p + k) as *const u8);
                        if c == 0 {
                            break;
                        }
                        name.push(c);
                    }
                    f(n, class, &name);
                } else {
                    w.bad += 1;
                }
                n = *((n + NODE_NEXT) as *const u64);
            }
        }
    }
    w
}

/// One walk of the exe's table under the exe's lock (None: the lock was not free within LOCK_WAIT; nothing read).
/// `f` runs under the lock: it must not log, wait or call exe code.
pub fn walk(mut f: impl FnMut(u32, &[u8])) -> Option<Walk> {
    walk_nodes(|_, c, n| f(c, n))
}

/// [`walk`] that also gives each node's address: `f(node, class, name bytes)` (the base pre-warm set, `baseset`).
pub fn walk_nodes(f: impl FnMut(u64, u32, &[u8])) -> Option<Walk> {
    let t0 = Instant::now();
    let mut tries = 0u32;
    while !crate::k32::critical_section_try_enter(LOCK) {
        if t0.elapsed() >= LOCK_WAIT {
            return None;
        }
        tries += 1;
        if tries < 64 {
            std::hint::spin_loop();
        } else {
            std::thread::sleep(Duration::from_micros(20));
        }
    }
    // SAFETY: under the exe's lock the table is not changed; its nodes and names are the exe's live objects
    let w = unsafe { walk_at_nodes(TABLE, BUCKETS, MAX_NODES, exe_ptr, f) };
    crate::k32::critical_section_leave(LOCK);
    Some(w)
}

/// The key of a (class, name) entry: FNV-1a 64 of the class's 4 bytes and the name.
pub fn key(class: u32, name: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in class.to_le_bytes().iter().chain(name) {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// The pre-warm list lines of late entries: those of the classes a zygote can query by name alone
/// (`daemon::PREWARM_CLASSES`), in the order found, each once, as `0xCLASS<TAB>name` lines. A name that cannot be one
/// list line (empty, or holding a tab, a line break or a NUL) is left out.
pub fn late_list_text(late: &[(u32, String)]) -> String {
    let mut once = HashSet::new();
    let mut out = String::new();
    for (c, n) in late {
        if !crate::daemon::PREWARM_CLASSES.contains(c) || n.is_empty() || n.contains(['\t', '\r', '\n', '\0']) {
            continue;
        }
        if once.insert((*c, n.as_str())) {
            out.push_str(&format!("{c:#x}\t{n}\n"));
        }
    }
    out
}

/// When in a match the trace looks at the cache.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// after the creation returned, before Ready goes out (the match-ready call, the first world, a PvE first step)
    BeforeReady,
    /// right after the host's AI started
    BotsStart,
    /// after a stepped DF, and the walk at the end
    InPlay,
}

impl Phase {
    fn text(self) -> &'static str {
        match self {
            Phase::BeforeReady => "before Ready",
            Phase::BotsStart => "at the bots' start",
            Phase::InPlay => "in play",
        }
    }
}

/// In play at most one walk per this much world time (ms): a count that moves every step then costs one walk a second.
pub const WALK_EVERY_MS: u32 = 1000;
/// `late load:` lines per match; the rest are counted.
pub const LATE_LINES: usize = 32;

/// Whether a check walks the table: when the count differs from the one of the last walk; in play only once
/// WALK_EVERY_MS of world time have passed since the last walk in play.
pub fn walk_due(phase: Phase, count: u32, walked: u32, t: u32, last_play_walk: Option<u32>) -> bool {
    count != walked && (phase != Phase::InPlay || last_play_walk.is_none_or(|w| t.saturating_sub(w) >= WALK_EVERY_MS))
}

/// The per-match summary line.
pub fn summary_line(by_phase: [usize; 3], at_ready: Option<u32>, at_end: u32, changes: u32, walks: u32, busy: u32, list: &str) -> String {
    let total: usize = by_phase.iter().sum();
    format!(
        "late loads: {total} after the creation ({} before Ready, {} at the bots' start, {} in play); cache entries {} at Ready, \
         {at_end} at the end, {changes} changes of the count, {walks} walks{}; list: {list}",
        by_phase[0],
        by_phase[1],
        by_phase[2],
        at_ready.map_or("?".to_string(), |c| c.to_string()),
        if busy > 0 { format!(" ({busy} skipped: the lock was busy)") } else { String::new() }
    )
}

/// One match's late-load trace (a match child's only thread uses it).
struct LateTrace {
    /// the keys of every entry seen: the cache at the end of the creation query, then every walk's
    seen: HashSet<u64>,
    /// the count at the last read and at the last walk
    last_read: u32,
    walked: u32,
    at_ready: Option<u32>,
    changes: u32,
    walks: u32,
    busy: u32,
    bad_logged: bool,
    last_play_walk: Option<u32>,
    last_t: u32,
    /// phase, world time, class, name of every entry found after the creation query
    late: Vec<(Phase, u32, u32, String)>,
}

impl LateTrace {
    fn check(&mut self, phase: Phase, t: u32) {
        self.last_t = t;
        let c = count();
        if c != self.last_read {
            self.changes += 1;
            self.last_read = c;
        }
        if walk_due(phase, c, self.walked, t, self.last_play_walk) {
            self.walk_now(phase, t);
        }
        if phase == Phase::BeforeReady {
            self.at_ready = Some(self.last_read);
        }
    }

    fn walk_now(&mut self, phase: Phase, t: u32) {
        if phase == Phase::InPlay {
            self.last_play_walk = Some(t);
        }
        let mut new = Vec::new();
        let seen = &mut self.seen;
        let r = walk(|c, n| {
            if seen.insert(key(c, n)) {
                new.push((c, String::from_utf8_lossy(n).into_owned()));
            }
        });
        let Some(w) = r else {
            self.busy += 1;
            return;
        };
        self.walks += 1;
        self.walked = w.count;
        self.last_read = w.count;
        if (w.bad > 0 || w.cut) && !self.bad_logged {
            self.bad_logged = true;
            hlog!("late loads: a walk at world time {t} skipped {} unreadable node(s) or name(s){}", w.bad, if w.cut { ", stopped at its node limit" } else { "" });
        }
        for (c, n) in new {
            if self.late.len() < LATE_LINES {
                hlog!("late load: world time {t}, {}: class {c:#x} {n}", phase.text());
            }
            self.late.push((phase, t, c, n));
        }
    }
}

/// Set in a match child before its creation ([`arm_late_trace`]).
static ARMED: AtomicBool = AtomicBool::new(false);
static TRACE: Mutex<Option<LateTrace>> = Mutex::new(None);

/// Diagnosis only: `HOST_LATE_TRACE=0` in the world host's environment: the name cache is never read (A/B runs).
pub fn late_trace_off() -> bool {
    std::env::var("HOST_LATE_TRACE").is_ok_and(|v| v == "0")
}

/// A match child, before its creation: watch the name cache from the end of the creation query on.
pub fn arm_late_trace() {
    if late_trace_off() {
        hlog!("late loads: not watched (diagnosis HOST_LATE_TRACE=0)");
        return;
    }
    ARMED.store(true, Ordering::Relaxed);
}

/// Right after a match creation's `pvp_match_resource` query returned the core: the cache as it is now is the
/// creation's; what is registered after it is a late load.
pub fn creation_query_done() {
    if !ARMED.load(Ordering::Relaxed) {
        return;
    }
    let t0 = Instant::now();
    let mut seen = HashSet::new();
    match walk(|c, n| {
        seen.insert(key(c, n));
    }) {
        Some(w) => {
            hlog!(
                "late loads: watching the name cache ({} entries at the end of the creation query, {} nodes{}; walk {} us)",
                w.count,
                w.nodes,
                if w.bad > 0 || w.cut { format!(", {} unreadable", w.bad) } else { String::new() },
                t0.elapsed().as_micros()
            );
            *TRACE.lock().unwrap_or_else(|e| e.into_inner()) = Some(LateTrace {
                seen,
                last_read: w.count,
                walked: w.count,
                at_ready: None,
                changes: 0,
                walks: 0,
                busy: 0,
                bad_logged: false,
                last_play_walk: None,
                last_t: 0,
                late: Vec::new(),
            });
        }
        None => hlog!("late loads: not watched: the name cache's lock was not free within {} ms at the end of the creation query", LOCK_WAIT.as_millis()),
    }
}

/// A check of the count at `phase`, world time `t` (one read; a walk when [`walk_due`]).
pub fn late_check(phase: Phase, t: u32) {
    if let Some(tr) = TRACE.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        tr.check(phase, t);
    }
}

/// The match's end: one more walk whatever the count did, the summary line, and the late entries of the list classes
/// into the project's own pre-warm list (`--record-prewarm`, the union of `daemon::grow_prewarm_list`). Once.
pub fn late_finish() {
    let Some(mut tr) = TRACE.lock().unwrap_or_else(|e| e.into_inner()).take() else { return };
    let t = tr.last_t;
    tr.walk_now(Phase::InPlay, t);
    let by = |p: Phase| tr.late.iter().filter(|l| l.0 == p).count();
    let by_phase = [by(Phase::BeforeReady), by(Phase::BotsStart), by(Phase::InPlay)];
    if tr.late.len() > LATE_LINES {
        hlog!("late loads: {} more not listed", tr.late.len() - LATE_LINES);
    }
    let late: Vec<(u32, String)> = tr.late.iter().map(|l| (l.2, l.3.clone())).collect();
    let text = late_list_text(&late);
    let list = match crate::bp::record_prewarm_path() {
        None => "not recorded (no own list)".to_string(),
        Some(_) if text.is_empty() => "0 new".to_string(),
        Some(path) => match crate::daemon::grow_prewarm_list(std::path::Path::new(&path), &text) {
            Ok((entries, added)) => {
                hlog!("pre-warm list {path}: late loads: {} entries, {added} new (list now {entries})", text.lines().count());
                format!("{added} new")
            }
            Err(e) => {
                hlog!("pre-warm list {path} not written: {e}");
                "not written".to_string()
            }
        },
    };
    hlog!("{}", summary_line(by_phase, tr.at_ready, tr.last_read, tr.changes, tr.walks, tr.busy, &list));
}

/// Held by a match child: its drop ends the trace ([`late_finish`]) on every way out of the match.
pub struct LateEnd;

impl Drop for LateEnd {
    fn drop(&mut self) {
        late_finish();
    }
}

/// A zygote's cache size at a point of its life (`zygote ready`, after a top-up): one walk, logged with its time
/// (nothing with HOST_LATE_TRACE=0).
pub fn zygote_cache_line(label: &str) {
    if late_trace_off() {
        return;
    }
    let t0 = Instant::now();
    let mut bytes = 0usize;
    match walk(|_, n| bytes += n.len()) {
        Some(w) => hlog!(
            "zygote: name cache [{label}]: {} entries ({} nodes{}), names {} kB; walk {} us",
            w.count,
            w.nodes,
            if w.bad > 0 || w.cut { format!(", {} unreadable", w.bad) } else { String::new() },
            bytes / 1024,
            t0.elapsed().as_micros()
        ),
        None => hlog!("zygote: name cache [{label}]: the lock was not free within {} ms; not read", LOCK_WAIT.as_millis()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A table of `buckets` heads in the exe's layout (a header word, the heads, the count); nodes and names on the
    /// heap.
    struct Fake {
        table: Vec<u64>,
        // boxed: a node keeps its address while the vector grows (the table and other nodes point at it)
        #[allow(clippy::vec_box)]
        nodes: Vec<Box<[u64; 4]>>,
        names: Vec<std::ffi::CString>,
    }

    impl Fake {
        fn new(buckets: usize, count: u32) -> Fake {
            let mut table = vec![0u64; buckets + 2];
            table[buckets + 1] = u64::from(count);
            Fake { table, nodes: Vec::new(), names: Vec::new() }
        }
        fn name(&mut self, s: &str) -> u64 {
            self.names.push(std::ffi::CString::new(s).unwrap());
            self.names.last().unwrap().as_ptr() as u64
        }
        /// a node of `class` with name pointer `name`, put first in bucket `b`; returns its address
        fn push(&mut self, b: usize, class: u32, name: u64) -> u64 {
            let n = Box::new([u64::from(class), 0, name, self.table[b + 1]]);
            let a = &*n as *const [u64; 4] as u64;
            self.table[b + 1] = a;
            self.nodes.push(n);
            a
        }
        fn base(&self) -> u64 {
            self.table.as_ptr() as u64
        }
        fn ok(&self) -> impl Fn(u64) -> bool + '_ {
            move |p| self.nodes.iter().any(|n| &**n as *const [u64; 4] as u64 == p) || self.names.iter().any(|s| s.as_ptr() as u64 == p)
        }
    }

    #[test]
    fn the_count_sits_right_after_the_last_bucket() {
        assert_eq!(COUNT, 0x200CB1278);
        assert!(exe_ptr(0x100_6903_13d8) && exe_ptr(0x2_0095_99b8));
        assert!(!exe_ptr(0) && !exe_ptr(0x7f00_0000_0000) && !exe_ptr(0x10));
    }

    #[test]
    fn a_walk_reads_every_chain_and_skips_what_it_cannot_read() {
        let mut f = Fake::new(4, 6);
        let a = f.name("resources/a.options");
        let b = f.name("resources/animations/b");
        let c = f.name("gameplay/c.user_animations");
        f.push(0, 0x1f, a);
        // a chain of three in bucket 2 (the last pushed comes first)
        f.push(2, 0x31, b);
        f.push(2, 0x8b, c);
        f.push(2, 0x43, 0); // a null name: skipped, counted
        // bucket 3 starts with a pointer outside the allowed range: not read, counted
        f.table[4] = 0x1234_5670;
        let mut got = Vec::new();
        let w = unsafe { walk_at(f.base(), 4, 100, f.ok(), |cl, n| got.push((cl, String::from_utf8_lossy(n).into_owned()))) };
        assert_eq!(got, vec![(0x1f, "resources/a.options".to_string()), (0x8b, "gameplay/c.user_animations".to_string()), (0x31, "resources/animations/b".to_string())]);
        assert_eq!(w, Walk { nodes: 4, bad: 2, cut: false, count: 6 });
    }

    #[test]
    fn a_walk_with_nodes_gives_each_node_its_address() {
        let mut f = Fake::new(2, 2);
        let a = f.name("a");
        let b = f.name("b");
        let na = f.push(0, 0x1f, a);
        let nb = f.push(1, 0x31, b);
        let mut got = Vec::new();
        unsafe { walk_at_nodes(f.base(), 2, 100, f.ok(), |node, c, n| got.push((node, c, n.to_vec()))) };
        assert_eq!(got, vec![(na, 0x1f, b"a".to_vec()), (nb, 0x31, b"b".to_vec())]);
    }

    #[test]
    fn a_chain_that_loops_stops_at_the_node_limit() {
        let mut f = Fake::new(2, 1);
        let a = f.name("x");
        let n = f.push(1, 0x1f, a);
        // the node's next is itself
        f.nodes[0][3] = n;
        let mut calls = 0;
        let w = unsafe { walk_at(f.base(), 2, 10, f.ok(), |_, _| calls += 1) };
        assert_eq!((w.nodes, w.cut, calls), (10, true, 10));
    }

    #[test]
    fn a_walk_names_only_entries_not_seen_before_each_once() {
        let mut f = Fake::new(2, 3);
        let a = f.name("resources/a.options");
        let b = f.name("resources/b.options");
        f.push(0, 0x1f, a);
        f.push(1, 0x1f, b);
        // the same name in another class is another entry; the same (class, name) twice is one
        f.push(1, 0x31, a);
        f.push(0, 0x1f, a);
        let mut seen: HashSet<u64> = [key(0x1f, b"resources/a.options")].into_iter().collect();
        let mut new = Vec::new();
        unsafe {
            walk_at(f.base(), 2, 100, f.ok(), |c, n| {
                if seen.insert(key(c, n)) {
                    new.push((c, String::from_utf8_lossy(n).into_owned()));
                }
            })
        };
        assert_eq!(new, vec![(0x31, "resources/a.options".to_string()), (0x1f, "resources/b.options".to_string())]);
        assert_ne!(key(0x1f, b"ab"), key(0x1f, b"ba"));
        assert_ne!(key(0x1f, b"a"), key(0x31, b"a"));
    }

    #[test]
    fn late_entries_go_into_the_list_only_for_the_list_classes_each_once() {
        let long = format!("resources/{}", "x".repeat(190));
        let late = vec![
            (0x1f, "resources/gameplay/a.options".to_string()),
            (0x43, "31".to_string()),
            (0x2f, "resources/animations/sub".to_string()),
            (0x31, long.clone()),
            (0x1f, "resources/gameplay/a.options".to_string()),
            (0x1f, "bad\tname".to_string()),
            (0x1f, String::new()),
        ];
        assert_eq!(late_list_text(&late), format!("0x1f\tresources/gameplay/a.options\n0x31\t{long}\n"));
        assert_eq!(long.len(), 200);
        assert_eq!(late_list_text(&[]), "");
        assert_eq!(late_list_text(&[(0x43, "31".to_string())]), "");
    }

    #[test]
    fn a_walk_is_due_on_a_changed_count_and_in_play_at_most_once_a_second() {
        assert!(!walk_due(Phase::BeforeReady, 5, 5, 0, None));
        assert!(walk_due(Phase::BeforeReady, 6, 5, 0, Some(0)));
        assert!(walk_due(Phase::BotsStart, 4, 5, 0, Some(0)));
        assert!(walk_due(Phase::InPlay, 6, 5, 850, None));
        assert!(!walk_due(Phase::InPlay, 6, 5, 1800, Some(850)));
        assert!(walk_due(Phase::InPlay, 6, 5, 1850, Some(850)));
        assert!(!walk_due(Phase::InPlay, 5, 5, 99_000, Some(850)));
    }

    #[test]
    fn the_end_line_also_says_when_nothing_was_loaded_late() {
        assert_eq!(
            summary_line([0, 0, 0], Some(5120), 5120, 0, 1, 0, "0 new"),
            "late loads: 0 after the creation (0 before Ready, 0 at the bots' start, 0 in play); cache entries 5120 at Ready, \
             5120 at the end, 0 changes of the count, 1 walks; list: 0 new"
        );
        assert_eq!(
            summary_line([0, 1, 2], None, 5123, 3, 3, 1, "1 new"),
            "late loads: 3 after the creation (0 before Ready, 1 at the bots' start, 2 in play); cache entries ? at Ready, \
             5123 at the end, 3 changes of the count, 3 walks (1 skipped: the lock was busy); list: 1 new"
        );
    }
}
