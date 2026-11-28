//! Hardware execution breakpoints on the host's own threads (perf_event_open, PERF_TYPE_BREAKPOINT with `sigtrap`):
//! they read exe state at an exact instruction without writing to the exe's code. Used for the player bind poses:
//! 0x200227883 is the render model's bind-pose call (render_model.vt[0x68] = 0x2004BA600 with the render model
//! instance, player+0x1DE28 and a count) and 0x200227887 the instruction after it, the same two labels the client
//! harness uses (bpre / bpost). The kernel sends a synchronous SIGTRAP with si_code TRAP_PERF to the thread that hit
//! the address; crash.rs passes it to `on_trap`, which copies the source block [[instance+0x3E0]+0x250] (at the call)
//! and the destination block (after it) into static buffers. `report` writes them out later.

use crate::hlog;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

pub const BIND_CALL: u64 = 0x200227883;
pub const BIND_AFTER: u64 = 0x200227887;
const BLOCK: usize = 69 * 0x40;
const MAX: usize = 8;

#[derive(Clone, Copy)]
struct Capture {
    tid: u32,
    instance: u64,
    count: u64,
    dest: u64,
    src: u64,
    player: u64,
    src_block: [u8; BLOCK],
    dest_block: [u8; BLOCK],
    done: bool,
}

const EMPTY: Capture =
    Capture { tid: 0, instance: 0, count: 0, dest: 0, src: 0, player: 0, src_block: [0; BLOCK], dest_block: [0; BLOCK], done: false };
static mut CAPS: [Capture; MAX] = [EMPTY; MAX];
static NEXT: AtomicUsize = AtomicUsize::new(0);
static HITS: AtomicUsize = AtomicUsize::new(0);
static mut FDS: [i32; 256] = [-1; 256];
static NFDS: AtomicUsize = AtomicUsize::new(0);

/// Bit 63 of sig_data marks a write watchpoint (the trap comes after the writing instruction).
const WATCH: u64 = 1 << 63;
static LOGGED: AtomicUsize = AtomicUsize::new(0);
static mut LOG_ADDRS: [u64; 8] = [0; 8];
static NLOG: AtomicUsize = AtomicUsize::new(0);
static BELOW_REG: AtomicUsize = AtomicUsize::new(0);

/// perf_event_attr (PERF_ATTR_SIZE_VER8, 136 bytes): type 5 (breakpoint), sample_period 1, flags exclude_kernel |
/// exclude_hv | inherit | remove_on_exec | sigtrap, bp_type 4 (execute) or 2 (write), bp_addr, bp_len 8.
fn attr(addr: u64) -> [u8; 136] {
    attr_kind(addr, 4, addr)
}

fn attr_kind(addr: u64, bp_type: u32, sig_data: u64) -> [u8; 136] {
    let mut a = [0u8; 136];
    let put32 = |a: &mut [u8; 136], o: usize, v: u32| a[o..o + 4].copy_from_slice(&v.to_le_bytes());
    let put64 = |a: &mut [u8; 136], o: usize, v: u64| a[o..o + 8].copy_from_slice(&v.to_le_bytes());
    put32(&mut a, 0, 5);
    put32(&mut a, 4, 136);
    put64(&mut a, 16, 1);
    let flags: u64 = (1 << 1) | (1 << 5) | (1 << 6) | (1 << 36) | (1 << 37);
    put64(&mut a, 40, flags);
    put32(&mut a, 52, bp_type);
    put64(&mut a, 56, addr);
    put64(&mut a, 64, 8);
    put64(&mut a, 120, sig_data); // si_perf_data names the breakpoint
    a
}

/// Can this process arm hardware breakpoints? One disabled execution breakpoint (it never fires) on the calling thread at
/// this function's own address, closed again. Err = the OS error: EACCES / EPERM when `kernel.perf_event_paranoid` is
/// above 2 for an unprivileged process, or a container's seccomp profile refuses perf_event_open (Docker: without
/// CAP_PERFMON).
pub fn probe() -> Result<(), String> {
    let mut at = attr_kind(probe as fn() -> Result<(), String> as usize as u64, 4, 0);
    at[40] |= 1; // flags bit 0: disabled
    let fd = unsafe { libc::syscall(libc::SYS_perf_event_open, at.as_ptr(), 0i32, -1i32, -1i32, 8u64) } as i32;
    if fd < 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    unsafe { libc::close(fd) };
    Ok(())
}

/// Arm execution breakpoints at `addrs` on every current thread of the process (inherited by threads they start).
pub fn arm(addrs: &[u64]) {
    arm_kind(addrs, 4, "breakpoints");
}

/// Exec breakpoints whose hits are logged with registers, stack return addresses and HOST_FAULT_DUMP regions.
pub fn arm_log(addrs: &[u64]) {
    if let Ok(r) = std::env::var("HOST_BP_BELOW") {
        let idx = match r.as_str() {
            "rax" => libc::REG_RAX, "rbx" => libc::REG_RBX, "rcx" => libc::REG_RCX, "rdx" => libc::REG_RDX,
            "rsi" => libc::REG_RSI, "rdi" => libc::REG_RDI, "r8" => libc::REG_R8, _ => libc::REG_R9,
        };
        BELOW_REG.store(idx as usize + 1, Ordering::SeqCst);
    }
    for &a in addrs {
        let i = NLOG.fetch_add(1, Ordering::SeqCst);
        if i < 8 {
            unsafe { LOG_ADDRS[i] = a };
        }
    }
    arm_kind(addrs, 4, "logging breakpoints");
}

/// 8-byte write watchpoints; every write is logged as one line (value after the write, the writing instruction's
/// address, return addresses).
pub fn arm_watch(addrs: &[u64]) {
    arm_kind(addrs, 2, "write watchpoints");
}

fn arm_kind(addrs: &[u64], bp_type: u32, what: &str) {
    for fd in open_kind(addrs, bp_type, what) {
        let i = NFDS.fetch_add(1, Ordering::SeqCst);
        if i < 256 {
            unsafe { FDS[i] = fd };
        }
    }
}

/// The breakpoints of [`arm_kind`] on every current thread; returns their descriptors. Closing a descriptor removes its
/// breakpoint, with the copies the thread's later threads inherited, and frees the debug register.
fn open_kind(addrs: &[u64], bp_type: u32, what: &str) -> Vec<i32> {
    let tids: Vec<i32> = std::fs::read_dir("/proc/self/task")
        .map(|d| d.filter_map(|e| e.ok()?.file_name().to_str()?.parse().ok()).collect())
        .unwrap_or_default();
    let mut fds = Vec::new();
    let mut err = String::new();
    for &tid in &tids {
        for &a in addrs {
            let at = attr_kind(a, bp_type, if bp_type == 2 { a | WATCH } else { a });
            let fd = unsafe { libc::syscall(libc::SYS_perf_event_open, at.as_ptr(), tid, -1i32, -1i32, 8u64) } as i32;
            if fd < 0 {
                err = format!("{}", std::io::Error::last_os_error());
                continue;
            }
            fds.push(fd);
        }
    }
    hlog!("{what}: {} armed ({} thread(s) x {:x?}){}", fds.len(), tids.len(), addrs, if err.is_empty() { String::new() } else { format!("; last error {err}") });
    fds
}

static mut WATCH_T: (u32, u32) = (0, 0);

/// HOST_WATCH_T=from,to limits watch logging to world step times in [from, to].
pub fn set_watch_time() {
    if let Some((a, b)) = std::env::var("HOST_WATCH_T").ok().and_then(|v| {
        let (a, b) = v.split_once(',')?;
        Some((a.parse::<u32>().ok()?, b.parse::<u32>().ok()?))
    }) {
        unsafe { WATCH_T = (a, b) };
        hlog!("watch logging limited to world step time {a}..{b}");
    }
}

/// The world step time W+0x180A98 (W = [0x201CF2CD8+0xF4640]+0x230), 0 before the match core exists.
unsafe fn world_time() -> u32 {
    let core = *((0x201CF2CD8u64 + 0xF4640) as *const u64);
    if core == 0 { 0 } else { *((core + 0x230 + 0x180A98) as *const u32) }
}

/// Exe-range values on the stack, from the stack pointer up, that follow a call instruction (e8 rel32, ff 15 / ff /2 forms), i.e.
/// return addresses, not stale function pointers. Up to `max` of them, within 0x2000 bytes.
unsafe fn call_rets(rsp: u64, max: usize) -> String {
    let (lo, hi) = (0x200001000u64, 0x2008A6000u64);
    let mut rets = String::new();
    let mut k = 0;
    let mut p = rsp;
    while p < rsp + 0x2000 && k < max {
        let v = *(p as *const u64);
        if v >= lo + 8 && v < hi {
            let b = |o: u64| *((v - o) as *const u8);
            let ff2 = |o: u64| b(o) == 0xFF && (b(o - 1) >> 3) & 7 == 2;
            if b(5) == 0xE8 || (b(6) == 0xFF && b(5) == 0x15) || ff2(2) || ff2(3) || ff2(4) || ff2(6) || ff2(7) {
                rets.push_str(&format!(" {v:#x}@{:#x}", p - rsp));
                k += 1;
            }
        }
        p += 8;
    }
    rets
}



/// HOST_MEMCPY_LOG=from,to: exec breakpoint at the exe's memcpy 0x2006F9910; every call while the world time is in
/// [from, to] logs (dest, src, len, caller) into a static table, written by `memcpy_report` (diagnosis: which live
/// field a serialized history-item byte comes from).
const MEMCPY: u64 = 0x2006F9910;
static mut MC_RANGE: (u32, u32) = (0, 0);
static mut MC_TAB: Option<Vec<(u64, u64, u64, u64, u32)>> = None;

pub fn arm_memcpy_log() {
    let Some((a, b)) = std::env::var("HOST_MEMCPY_LOG").ok().and_then(|v| {
        let (a, b) = v.split_once(',')?;
        Some((a.parse::<u32>().ok()?, b.parse::<u32>().ok()?))
    }) else { return };
    unsafe {
        MC_RANGE = (a, b);
        MC_TAB = Some(Vec::with_capacity(400_000));
    }
    arm_kind(&[MEMCPY], 4, "memcpy logging breakpoints (HOST_MEMCPY_LOG)");
}

unsafe fn memcpy_hit(g: &[libc::greg_t], rip: u64) -> bool {
    if rip != MEMCPY {
        return false;
    }
    let (a, b) = *std::ptr::addr_of!(MC_RANGE);
    let t = world_time();
    if t >= a && t <= b {
        if let Some(v) = (*std::ptr::addr_of_mut!(MC_TAB)).as_mut() {
            if v.len() < v.capacity() {
                let ret = *(g[libc::REG_RSP as usize] as u64 as *const u64);
                v.push((g[libc::REG_RCX as usize] as u64, g[libc::REG_RDX as usize] as u64, g[libc::REG_R8 as usize] as u64, ret, t));
            }
        }
    }
    true
}

pub fn memcpy_report(out: &str) {
    if let Some(v) = unsafe { (*std::ptr::addr_of!(MC_TAB)).as_ref() } {
        let mut s = String::new();
        for (d, src, n, ret, t) in v {
            s.push_str(&format!("{t} {d:#x} {src:#x} {n} {ret:#x}
"));
        }
        let _ = std::fs::write(format!("{out}/memcpy_log.txt"), s);
        hlog!("memcpy log: {} call(s) -> {out}/memcpy_log.txt", v.len());
    }
}

/// HOST_QUERY_LOG=1: exec breakpoint at the resource manager's query creation 0x2000BC810 (every query, the cooks'
/// sub-queries too; its params: +0 requests {char* name, u32 class, pad} (16 bytes each), +0x10 u32 count). Every
/// request (name, class, the calling thread, the return address) goes into a static table that `query_report` writes
/// as queries.tsv (zygote pre-warm: which resources a match creation asks for).
const QUERY: u64 = 0x2000BC810;
static mut QL_TAB: Option<Vec<([u8; 120], u32, u32, u64)>> = None;
/// Taken by `query_hit` around its pushes: two exe threads (the world thread, :resources manager) can create queries at
/// the same moment.
static QL_LOCK: AtomicBool = AtomicBool::new(false);

/// HOST_QUERY_LOG=1 (diagnosis): the query log for the whole run, from `--debug-at` on. Pre-warm recording arms its own
/// breakpoint per match creation ([`record_start`]).
pub fn arm_query_log() {
    if std::env::var("HOST_QUERY_LOG").is_err() {
        return;
    }
    unsafe { QL_TAB = Some(Vec::with_capacity(60_000)) };
    arm_kind(&[QUERY], 4, "query logging breakpoints (HOST_QUERY_LOG)");
}

unsafe fn query_hit(g: &[libc::greg_t], rip: u64, tid: u32) -> bool {
    if rip != QUERY {
        return false;
    }
    let p = g[libc::REG_RDX as usize] as u64;
    let ret = *(g[libc::REG_RSP as usize] as u64 as *const u64);
    // bounded: a holder that never returns (a fault in the handler) costs this query's entries, never a hang
    for _ in 0..10_000 {
        if QL_LOCK.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_ok() {
            unsafe { query_push(p, ret, tid) };
            QL_LOCK.store(false, Ordering::Release);
            return true;
        }
        unsafe { libc::sched_yield() };
    }
    true
}

/// One query's requests into the table (signal context, under `QL_LOCK`; the table never grows past its capacity).
unsafe fn query_push(p: u64, ret: u64, tid: u32) {
    // edition 2024: every unsafe operation of an unsafe fn needs its own block (unsafe_op_in_unsafe_fn)
    unsafe {
        if let Some(v) = (*std::ptr::addr_of_mut!(QL_TAB)).as_mut()
            && p > 0x10000
        {
            let reqs = *(p as *const u64);
            let n = (*((p + 0x10) as *const u32)).min(4096);
            for i in 0..n as u64 {
                if v.len() >= v.capacity() || reqs < 0x10000 {
                    break;
                }
                let name = *((reqs + i * 16) as *const u64);
                let class = *((reqs + i * 16 + 8) as *const u32);
                let mut b = [0u8; 120];
                if name > 0x10000 {
                    for k in 0..119 {
                        let c = *((name + k) as *const u8);
                        if c == 0 {
                            break;
                        }
                        b[k as usize] = c;
                    }
                }
                v.push((b, class, tid, ret));
            }
        }
    }
}

/// The query table from entry `from` on as queries.tsv text (header, then class, name, thread, caller per request), and
/// the number of requests in it.
fn query_text(from: usize) -> Option<(String, usize)> {
    let v = unsafe { (*std::ptr::addr_of!(QL_TAB)).as_ref() }?;
    let mut s = String::from("class\tname\ttid\tcaller\n");
    let mut n = 0;
    for (b, c, t, r) in v.iter().skip(from) {
        let k = b.iter().position(|&x| x == 0).unwrap_or(b.len());
        s.push_str(&format!("{c:#x}\t{}\t{t}\t{r:#x}\n", String::from_utf8_lossy(&b[..k])));
        n += 1;
    }
    Some((s, n))
}

pub fn query_report(out: &str) {
    if let Some((s, n)) = query_text(0) {
        let _ = std::fs::write(format!("{out}/queries.tsv"), s);
        hlog!("query log: {n} request(s) -> {out}/queries.tsv");
    }
}

/// `--record-prewarm PATH`: the project's own pre-warm list, which every match creation grows (the daemon passes it to
/// every zygote of a project with a `prewarm_dir` list, whether that list exists yet or not).
pub fn record_prewarm_path() -> Option<String> {
    crate::cmdline::value("--record-prewarm")
}

/// The current match creation's recording: the descriptors of its query breakpoints (closed by [`record_prewarm`]) and
/// the table entry its requests start at.
static REC: std::sync::Mutex<(Vec<i32>, usize)> = std::sync::Mutex::new((Vec::new(), 0));

/// With `--record-prewarm`, just before a match creation's `pvp_match_resource` query: log the creation's resource
/// requests. The breakpoint at the query creation is armed here, in the match's own process, and closed again by
/// [`record_prewarm`] right after the query, so it holds a debug register (x86-64 has four per thread) only during
/// the query and never competes with the kill log, the bots' scene and assert breakpoints armed later. The handler only
/// reads registers and exe memory and writes the host's own table (`query_hit`): no exe byte changes.
pub fn record_start() {
    if record_prewarm_path().is_none() {
        return;
    }
    let have = unsafe { (*std::ptr::addr_of!(QL_TAB)).as_ref().map(|v| v.len()) };
    if let Some(len) = have {
        // HOST_QUERY_LOG armed the breakpoint for the whole run: record only this creation's part of the table
        REC.lock().unwrap().1 = len;
        return;
    }
    unsafe { QL_TAB = Some(Vec::with_capacity(60_000)) };
    let fds = open_kind(&[QUERY], 4, "pre-warm recording breakpoints");
    *REC.lock().unwrap() = (fds, 0);
}

/// After a match creation's `pvp_match_resource` query (`created` = it returned the core): close the recording's
/// breakpoints, then grow the project's pre-warm list at the `--record-prewarm` path by the shareable resources the
/// creation asked for ([`crate::daemon::prewarm_list`], [`crate::daemon::grow_prewarm_list`]: the union, written
/// through a temporary name and a rename under a lock, so zygotes that start later read a whole list).
pub fn record_prewarm(created: bool) {
    let Some(path) = record_prewarm_path() else { return };
    let (fds, from) = std::mem::take(&mut *REC.lock().unwrap());
    for &fd in &fds {
        unsafe { libc::close(fd) };
    }
    let ours = !fds.is_empty();
    // under the handler's lock: a handler already under way on another thread finishes first
    let table = under_query_lock(|| {
        let t = if created { query_text(from) } else { None };
        if ours {
            // the table was this recording's own (not HOST_QUERY_LOG's): freed, so the match does not keep its pages
            unsafe { QL_TAB = None };
        }
        t
    })
    .flatten();
    if ours {
        hlog!("pre-warm recording breakpoints: {} closed", fds.len());
    }
    let Some((text, n)) = table else { return };
    let list = crate::daemon::prewarm_list(&text, &crate::daemon::PREWARM_CLASSES);
    let mine = list.lines().count();
    // the base pre-warm set's entries were found cached: what their cooks ask for joins as a cold creation records it
    let list = crate::baseset::with_closure(&list);
    let closure = list.lines().count() - mine;
    if closure > 0 {
        hlog!("pre-warm list {path}: {closure} entries the base set's cooks asked for join this creation's {mine}");
    }
    match crate::daemon::grow_prewarm_list(std::path::Path::new(&path), &list) {
        Ok((entries, 0)) => hlog!("pre-warm list {path}: nothing new ({entries} entries; this creation {mine} resources of {n} requests)"),
        Ok((entries, added)) => hlog!("pre-warm list recorded: {path}: {entries} entries, {added} new (this creation {mine} resources of {n} requests)"),
        Err(e) => hlog!("pre-warm list {path} not written: {e}"),
    }
}

/// Run `f` under `QL_LOCK` (a bounded wait, as the handler's), so no handler pushes meanwhile; None when the lock never
/// came free.
fn under_query_lock<T>(f: impl FnOnce() -> T) -> Option<T> {
    for _ in 0..10_000 {
        if QL_LOCK.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_ok() {
            let r = f();
            QL_LOCK.store(false, Ordering::Release);
            return Some(r);
        }
        unsafe { libc::sched_yield() };
    }
    None
}

/// The base pre-warm set's level-load check (`baseset`): the exe's cache lookup 0x2000CAF70 runs its find 0x2000C2A10
/// and, when the find returned a node, branches to this address (its only jump here, 0x2000CB003), where the found node
/// is still in the register the find's result was loaded into. From here the lookup either attaches the request to
/// the node's request in flight or hands out the node's finished resource: every found entry passes here, a missed one
/// never. The other two calls of the find (0x2000C1ED8 before an erase, 0x2000CABF7 in the completion path) serve no
/// request.
pub const FOUND_AT: u64 = 0x2000CB03D;
static FOUND_SET: std::sync::atomic::AtomicPtr<u64> = std::sync::atomic::AtomicPtr::new(std::ptr::null_mut());
static FOUND_LEN: AtomicUsize = AtomicUsize::new(0);
static FOUND_N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static FOUND_HITS: AtomicUsize = AtomicUsize::new(0);
/// The first found nodes of the set (the count goes on in FOUND_HITS).
const FOUND_KEEP: usize = 32;
static FOUND_HIT_NODES: [std::sync::atomic::AtomicU64; FOUND_KEEP] = [const { std::sync::atomic::AtomicU64::new(0) }; FOUND_KEEP];

/// Arm the check: an execution breakpoint at [`FOUND_AT`] on every thread; a found node that is one of `nodes`
/// (ascending) is counted and kept. Returns the breakpoints' descriptors and the number of threads they were meant for.
pub fn arm_found_check(nodes: &'static [u64]) -> (Vec<i32>, usize) {
    FOUND_N.store(0, Ordering::SeqCst);
    FOUND_HITS.store(0, Ordering::SeqCst);
    FOUND_LEN.store(nodes.len(), Ordering::SeqCst);
    FOUND_SET.store(nodes.as_ptr() as *mut u64, Ordering::SeqCst);
    let threads = std::fs::read_dir("/proc/self/task").map(|d| d.count()).unwrap_or(0);
    (open_kind(&[FOUND_AT], 4, "level-load check breakpoints (base pre-warm set)"), threads)
}

/// Close the check's breakpoints. Returns (found entries, the first found nodes of the set, found nodes of the set).
pub fn end_found_check(fds: Vec<i32>) -> (u64, Vec<u64>, usize) {
    for fd in fds {
        unsafe { libc::close(fd) };
    }
    FOUND_SET.store(std::ptr::null_mut(), Ordering::SeqCst);
    let hits = FOUND_HITS.load(Ordering::SeqCst);
    let kept = (0..hits.min(FOUND_KEEP)).map(|i| FOUND_HIT_NODES[i].load(Ordering::SeqCst)).collect();
    (FOUND_N.load(Ordering::SeqCst), kept, hits)
}

/// The check's handler (signal context): reads one register and the host's own sorted set, writes host atomics only.
unsafe fn found_hit(g: &[libc::greg_t], rip: u64) -> bool {
    if rip != FOUND_AT {
        return false;
    }
    let p = FOUND_SET.load(Ordering::SeqCst);
    if p.is_null() {
        return true;
    }
    FOUND_N.fetch_add(1, Ordering::SeqCst);
    let node = g[libc::REG_RDX as usize] as u64;
    // SAFETY: the set is the base's, inherited and never changed or freed ('static), FOUND_LEN long
    let set = unsafe { std::slice::from_raw_parts(p as *const u64, FOUND_LEN.load(Ordering::SeqCst)) };
    if set.binary_search(&node).is_ok() {
        let i = FOUND_HITS.fetch_add(1, Ordering::SeqCst);
        if i < FOUND_KEEP {
            FOUND_HIT_NODES[i].store(node, Ordering::SeqCst);
        }
    }
    true
}

/// HOST_ARMW=addr,reg,off,from: an exec breakpoint at `addr`; its first hit at world time >= `from` arms an 8-byte write
/// watchpoint on reg+off (reg: rax rbx rcx rdx rsi rdi rbp, r8 to r15) on that thread. For live fields whose object address
/// changes from run to run (float drift diagnosis); the writes are logged like --watch, gated by HOST_WATCH_T.
static mut ARMW: (u64, usize, u64, u32, bool) = (0, 0, 0, 0, false);
static mut ARMW_SEEN: [u64; 2] = [0; 2];

pub fn arm_armw() {
    let Ok(v) = std::env::var("HOST_ARMW") else { return };
    let f: Vec<&str> = v.split(',').collect();
    if f.len() != 4 {
        return;
    }
    let hex = |x: &str| u64::from_str_radix(x.trim_start_matches("0x"), 16).unwrap_or(0);
    let reg = match f[1] {
        "rax" => libc::REG_RAX, "rbx" => libc::REG_RBX, "rcx" => libc::REG_RCX, "rdx" => libc::REG_RDX,
        "rsi" => libc::REG_RSI, "rdi" => libc::REG_RDI, "rbp" => libc::REG_RBP, "r8" => libc::REG_R8,
        "r9" => libc::REG_R9, "r10" => libc::REG_R10, "r11" => libc::REG_R11, "r12" => libc::REG_R12,
        "r13" => libc::REG_R13, "r14" => libc::REG_R14, _ => libc::REG_R15,
    } as usize;
    let (a, off, from) = (hex(f[0]), hex(f[2]), f[3].parse::<u32>().unwrap_or(0));
    unsafe { ARMW = (a, reg, off, from, false) };
    set_watch_time();
    arm_kind(&[a], 4, "arm-watch breakpoints (HOST_ARMW)");
}

unsafe fn armw_hit(g: &[libc::greg_t], rip: u64) -> bool {
    let (a, reg, off, from, done) = *std::ptr::addr_of!(ARMW);
    if a == 0 || rip != a {
        return false;
    }
    let seen = &mut *std::ptr::addr_of_mut!(ARMW_SEEN);
    if !done && world_time() >= from && !seen.contains(&(g[reg] as u64)) {
        // the first two distinct objects seen at this address (e.g. both players' components)
        let slot = if seen[0] == 0 { 0 } else { 1 };
        seen[slot] = g[reg] as u64;
        // HOST_ARMW_POKE=hex: instead of a watchpoint, write that u32 at reg+off (diagnosis data write, once per object)
        if let Some(v) = std::env::var("HOST_ARMW_POKE").ok().and_then(|x| u32::from_str_radix(x.trim_start_matches("0x"), 16).ok()) {
            let a2 = (g[reg] as u64).wrapping_add(off);
            let old = *(a2 as *const u32);
            *(a2 as *mut u32) = v;
            ARMW.4 = slot == 1;
            crate::log::line(&format!("armw: poke at {a:#x} t {}: [{a2:#x}] {old:#x} -> {v:#x}", world_time()));
            return true;
        }
        let w = (g[reg] as u64).wrapping_add(off) & !7;
        let tid = libc::syscall(libc::SYS_gettid) as i32;
        let at = attr_kind(w, 2, w | WATCH);
        let fd = libc::syscall(libc::SYS_perf_event_open, at.as_ptr(), tid, -1i32, -1i32, 8u64) as i32;
        ARMW.4 = slot == 1;
        crate::log::line(&format!("armw: at {a:#x} t {}: write watchpoint on {w:#x} = {:#x} (reg value {:#x} + {off:#x}) tid {tid}: fd {fd}", world_time(), *(w as *const u64), g[reg]));
    }
    true
}

/// Called from the SIGTRAP handler for si_code TRAP_PERF (async-signal context: no allocation, no locks).
pub unsafe fn on_trap(g: &[libc::greg_t], data: u64, uc: *const libc::ucontext_t) {
    let rip = g[libc::REG_RIP as usize] as u64;
    let tid = libc::syscall(libc::SYS_gettid) as u32;
    HITS.fetch_add(1, Ordering::SeqCst);
    if rip == crate::kills::KILL_ENTRY {
        crate::kills::on_trap(g);
        return;
    }
    if crate::bots::on_assert_trap(rip, g) {
        return;
    }
    if rip == CC_UPDATE {
        cc_dump(g, uc);
        return;
    }
    if data & WATCH != 0 {
        let a = data & !WATCH;
        // HOST_WATCH_T=from,to (read at arm time): log only writes while the world step time is in that range
        let (wf, wt) = *std::ptr::addr_of!(WATCH_T);
        if wt != 0 {
            let t = world_time();
            if t < wf || t > wt {
                return;
            }
        }
        let n = LOGGED.fetch_add(1, Ordering::SeqCst);
        if n < 20000 {
            let rets = call_rets(g[libc::REG_RSP as usize] as u64, 12);
            crate::log::line(&format!(
                "watch {n}: [{a:#x}] = {:#x} after rip {rip:#x} tid {tid} t {}; rcx {:#x} rdx {:#x} r8 {:#x} rbx {:#x} rsi {:#x} rdi {:#x}; rets{rets}",
                *(a as *const u64), world_time(),
                g[libc::REG_RCX as usize], g[libc::REG_RDX as usize], g[libc::REG_R8 as usize],
                g[libc::REG_RBX as usize], g[libc::REG_RSI as usize], g[libc::REG_RDI as usize]
            ));
        }
        return;
    }
    if cc_exit(g, rip) {
        return;
    }
    if armw_hit(g, rip) {
        return;
    }
    if memcpy_hit(g, rip) {
        return;
    }
    if unsafe { found_hit(g, rip) } {
        return;
    }
    if query_hit(g, rip, tid) {
        return;
    }
    if cc_frame(g, uc, rip) {
        return;
    }
    let nlog = NLOG.load(Ordering::SeqCst).min(8);
    if (&*std::ptr::addr_of!(LOG_ADDRS))[..nlog].contains(&rip) {
        // HOST_BP_BELOW=reg (read at arm time): log only hits where that register is below the low area (1 TB)
        let r = BELOW_REG.load(Ordering::SeqCst);
        if r != 0 && (g[r - 1] as u64) >= crate::mem::LOW_START as u64 {
            return;
        }
        let s = crate::crash::describe(&format!("BREAKPOINT at {rip:#x} tid {tid}"), g, 48);
        crate::log::line(&s);
        crate::log::flush();
        return;
    }
    let caps = &mut *std::ptr::addr_of_mut!(CAPS);
    if rip == BIND_CALL {
        let i = NEXT.fetch_add(1, Ordering::SeqCst);
        if i >= MAX {
            return;
        }
        let c = &mut caps[i];
        c.tid = tid;
        c.instance = g[libc::REG_RCX as usize] as u64;
        c.dest = g[libc::REG_RDX as usize] as u64;
        c.count = g[libc::REG_R8 as usize] as u64 & 0xFFFF_FFFF;
        let inner = if c.instance != 0 { *((c.instance + 0x3E0) as *const u64) } else { 0 };
        c.src = if inner != 0 { *((inner + 0x250) as *const u64) } else { 0 };
        if c.src != 0 {
            std::ptr::copy_nonoverlapping(c.src as *const u8, c.src_block.as_mut_ptr(), BLOCK);
        }
    } else if rip == BIND_AFTER {
        // the latest open capture of this thread
        let n = NEXT.load(Ordering::SeqCst).min(MAX);
        for c in caps[..n].iter_mut().rev() {
            if c.tid == tid && !c.done {
                c.player = g[libc::REG_RSI as usize] as u64;
                if c.dest != 0 {
                    std::ptr::copy_nonoverlapping(c.dest as *const u8, c.dest_block.as_mut_ptr(), BLOCK);
                }
                c.done = true;
                break;
            }
        }
    }
}

/// Write the captures to <out>/bindbp_NNN_{src,p<player>_c<count>}.bin and log them.
pub fn report(out: &str) {
    let n = NEXT.load(Ordering::SeqCst).min(MAX);
    hlog!("breakpoints: {} hit(s), {} bind-pose capture(s)", HITS.load(Ordering::SeqCst), n);
    let caps = unsafe { &*std::ptr::addr_of!(CAPS) };
    for (i, c) in caps[..n].iter().enumerate() {
        let sp = format!("{out}/bindbp_{:03}_src.bin", i + 1);
        let dp = format!("{out}/bindbp_{:03}_p{:x}_c{}.bin", i + 1, c.player, c.count);
        let _ = std::fs::write(&sp, c.src_block);
        let _ = std::fs::write(&dp, c.dest_block);
        let nz = c.dest_block.iter().filter(|&&b| b != 0).count();
        hlog!(
            "bind capture {}: tid {} instance {:#x} count {} src {:#x} dest {:#x} (player {:#x}, player+0x1DE28 {:#x}), after-call seen {}, dest non-zero {nz} -> {dp}",
            i + 1, c.tid, c.instance, c.count, c.src, c.dest, c.player, c.player + 0x1DE28, c.done
        );
    }
}

/// The character controller update 0x2005F3AF0 (arguments: the controller, the step's time delta; position at +0xD0,
/// copied to +0x60 / +0x70). HOST_CC_DUMP=from,to (world step time W+0x180A98, W = [0x201CF2CD8+0xF4640]+0x230)
/// writes the controller's first 0x260 bytes at entry to <out>/cc/cc_<time>_<controller>.bin and logs the time delta
/// and MXCSR, the same record the client harness label `cc` takes (float drift diagnosis).
pub const CC_UPDATE: u64 = 0x2005F3AF0;
static CC_N: AtomicUsize = AtomicUsize::new(0);
static mut CC_OUT: Option<String> = None;
static mut CC_RANGE: (u32, u32) = (0, 0);
/// HOST_CC_LEN (hex, default 0x260): bytes dumped per controller. HOST_CC_EXIT=1: also dump at the update's return
/// (cc_exit_<time>_<controller>.bin): the entry hit arms an exec breakpoint at its return address on this thread, and
/// the hit whose stack pointer is the entry's + 8 is the update's own return.
static mut CC_LEN: usize = 0x260;
static mut CC_EXIT: bool = false;
static mut CC_PEND: (u64, u64, u32) = (0, 0, 0); // (stack pointer after return, controller, time)
static mut CC_RETS: [u64; 4] = [0; 4];
static mut CC_WATCHED: [u64; 2] = [0; 2];

pub fn arm_cc(out: &str, from: u32, to: u32) {
    unsafe {
        CC_OUT = Some(format!("{out}/cc"));
        CC_RANGE = (from, to);
        if let Some(n) = std::env::var("HOST_CC_LEN").ok().and_then(|v| usize::from_str_radix(v.trim_start_matches("0x"), 16).ok()) {
            CC_LEN = n;
        }
        CC_EXIT = std::env::var("HOST_CC_EXIT").map(|v| v == "1").unwrap_or(false);
        hlog!("controller dump: time {from}..{to}, {:#x} bytes, exit dump {}", *std::ptr::addr_of!(CC_LEN), *std::ptr::addr_of!(CC_EXIT));
    }
    let _ = std::fs::create_dir_all(format!("{out}/cc"));
    arm_kind(&[CC_UPDATE], 4, "controller-update breakpoints");
    // HOST_CCF=addr[,addr]: frame dumps inside the update (the client harness label `ccf`)
    let ccf: Vec<u64> = std::env::var("HOST_CCF")
        .map(|v| v.split(',').filter_map(|x| u64::from_str_radix(x.trim_start_matches("0x"), 16).ok()).collect())
        .unwrap_or_default();
    if !ccf.is_empty() {
        unsafe {
            let a = &mut *std::ptr::addr_of_mut!(CCF_ADDRS);
            for (i, &x) in ccf.iter().take(2).enumerate() {
                a[i] = x;
            }
        }
        arm_kind(&ccf[..ccf.len().min(2)], 4, "controller frame-dump breakpoints");
    }
}

/// HOST_CCF frame dump: at an exec breakpoint inside the update, write cc/ccf_<addr>_<time>_<controller>.bin = 0x1D0
/// bytes of the update's stack frame (the hit fraction at frame +0x1B0) then the 16 vector registers (16 bytes each),
/// the record the client harness label `ccf` writes.
static mut CCF_ADDRS: [u64; 2] = [0; 2];

unsafe fn cc_frame(g: &[libc::greg_t], uc: *const libc::ucontext_t, rip: u64) -> bool {
    if !(*std::ptr::addr_of!(CCF_ADDRS)).contains(&rip) {
        return false;
    }
    let t = world_time();
    let (from, to) = *std::ptr::addr_of!(CC_RANGE);
    if t < from || t > to {
        return true;
    }
    let rsp = g[libc::REG_RSP as usize] as u64;
    let rbx = g[libc::REG_RBX as usize] as u64;
    let mut out = Vec::with_capacity(0x2D0);
    out.extend_from_slice(std::slice::from_raw_parts(rsp as *const u8, 0x1D0));
    let fp = (*uc).uc_mcontext.fpregs;
    for i in 0..16 {
        let x = if fp.is_null() { [0u32; 4] } else { (*fp)._xmm[i].element };
        for v in x {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    if let Some(dir) = (*std::ptr::addr_of!(CC_OUT)).as_ref() {
        let _ = std::fs::write(format!("{dir}/ccf_{rip:x}_{t}_{rbx:x}.bin"), out);
    }
    true
}

/// The exit hit of HOST_CC_EXIT; true when the hit address is one of the armed return addresses (handled or ignored).
unsafe fn cc_exit(g: &[libc::greg_t], rip: u64) -> bool {
    if !CC_EXIT || !(*std::ptr::addr_of!(CC_RETS)).contains(&rip) {
        return false;
    }
    let (rsp, rcx, t) = *std::ptr::addr_of!(CC_PEND);
    if rsp != 0 && g[libc::REG_RSP as usize] as u64 == rsp {
        CC_PEND = (0, 0, 0);
        if let Some(dir) = (*std::ptr::addr_of!(CC_OUT)).as_ref() {
            let blk = std::slice::from_raw_parts(rcx as *const u8, CC_LEN);
            let _ = std::fs::write(format!("{dir}/cc_exit_{t}_{rcx:x}.bin"), blk);
        }
    }
    true
}

unsafe fn cc_dump(g: &[libc::greg_t], uc: *const libc::ucontext_t) {
    let core = *((0x201CF2CD8u64 + 0xF4640) as *const u64);
    if core == 0 {
        return;
    }
    let t = *((core + 0x230 + 0x180A98) as *const u32);
    let (from, to) = *std::ptr::addr_of!(CC_RANGE);
    if t < from || t > to {
        return;
    }
    let rcx = g[libc::REG_RCX as usize] as u64;
    let rsp = g[libc::REG_RSP as usize] as u64;
    let ret = *(rsp as *const u64);
    let fp = (*uc).uc_mcontext.fpregs;
    let (mxcsr, x1) = if fp.is_null() { (0, [0u32; 4]) } else { ((*fp).mxcsr, (*fp)._xmm[1].element) };
    let f: Vec<f32> = x1.iter().map(|&v| f32::from_bits(v)).collect();
    let n = CC_N.fetch_add(1, Ordering::SeqCst);
    if let Some(dir) = (*std::ptr::addr_of!(CC_OUT)).as_ref() {
        let blk = std::slice::from_raw_parts(rcx as *const u8, CC_LEN);
        let _ = std::fs::write(format!("{dir}/cc_{t}_{rcx:x}.bin"), blk);
    }
    // HOST_CC_WATCH=1: the first in-range entry of each controller (up to 2) arms an 8-byte write watchpoint on its
    // +0xD0 (x, y) on this thread; controller addresses differ between runs, so they are taken here, not from args.
    if std::env::var("HOST_CC_WATCH").map(|v| v == "1").unwrap_or(false) {
        let w = &mut *std::ptr::addr_of_mut!(CC_WATCHED);
        if !w.contains(&rcx) {
            if let Some(slot) = w.iter_mut().find(|r| **r == 0) {
                *slot = rcx;
                // HOST_CC_WATCH_OFF (hex, default D0): the watched controller offset
                let off = std::env::var("HOST_CC_WATCH_OFF").ok().and_then(|v| u64::from_str_radix(v.trim_start_matches("0x"), 16).ok()).unwrap_or(0xD0);
                let a = rcx + off;
                let tid = libc::syscall(libc::SYS_gettid) as i32;
                let at = attr_kind(a, 2, a | WATCH);
                let fd = libc::syscall(libc::SYS_perf_event_open, at.as_ptr(), tid, -1i32, -1i32, 8u64) as i32;
                crate::log::line(&format!("cc: write watchpoint on {a:#x} (controller {rcx:#x} +{off:#x}) tid {tid}: fd {fd}"));
            }
        }
    }
    if CC_EXIT {
        let rets = &mut *std::ptr::addr_of_mut!(CC_RETS);
        if !rets.contains(&ret) {
            if let Some(slot) = rets.iter_mut().find(|r| **r == 0) {
                let tid = libc::syscall(libc::SYS_gettid) as i32;
                let at = attr_kind(ret, 4, ret);
                let fd = libc::syscall(libc::SYS_perf_event_open, at.as_ptr(), tid, -1i32, -1i32, 8u64) as i32;
                *slot = ret;
                crate::log::line(&format!("cc: exit breakpoint at return address {ret:#x} on tid {tid}: fd {fd}"));
            }
        }
        CC_PEND = (rsp + 8, rcx, t);
    }
    if n < 400 {
        crate::log::line(&format!("cc {n}: tid {} time {t} controller {rcx:#x} ret {ret:#x} xmm1 {:08x?} {f:?} mxcsr {mxcsr:#06x}", libc::syscall(libc::SYS_gettid), x1));
    }
}
