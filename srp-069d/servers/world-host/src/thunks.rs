//! Import thunks. Every IAT slot (static + delay) and every GetProcAddress answer points at a small
//! generated thunk:
//!
//! ```text
//!   push rcx; push rdx; push r8; push r9; sub rsp,0x28     ; keep the 4 register args
//!   mov ecx, <index>; mov rdx, [rsp+0x48]                  ; rdx = caller's return address
//!   movabs rax, note; call rax                              ; note() counts the call, returns impl
//!   add rsp,0x28; pop r9; pop r8; pop rdx; pop rcx
//!   jmp rax                                                 ; tail-jump into the host stub
//! ```
//!
//! `note` logs the first call of each import and aborts (after logging DLL!function and the
//! return address) when the import has no host implementation. Stack args (5th+) are untouched
//! because the thunk jumps, so host stubs are plain `extern "win64"` functions.

use crate::hlog;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, AtomicU64, AtomicUsize, Ordering};

pub const MAX_SLOTS: usize = 4096;
/// Log every import call (except the hottest few) - switched on around the step under investigation.
pub static TRACE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
const THUNK_SIZE: usize = 64;

pub struct Slot {
    pub dll: OnceLock<String>,
    pub name: OnceLock<String>,
    pub imp: AtomicUsize,
    pub count: AtomicU64,
    pub first_ret: AtomicU64,
    /// The slot's `--escape-at` / `--end-at` match, decided once ([`SlotFlags`]); 0 = not decided yet.
    flags: AtomicU8,
}

/// What a slot's import is for the run's `--escape-at` and `--end-at` lists. Decided once per slot after the lists are
/// set (run::hook_lists_ready), so a call does no string work: the lists name an import by "DLL!name", "name" or
/// "DLL", which needs a formatted string per test.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SlotFlags(u8);

impl SlotFlags {
    const DECIDED: u8 = 1;
    const ESCAPE: u8 = 2;
    const END_AT: u8 = 4;

    pub fn new(escape: bool, end_at: bool) -> SlotFlags {
        SlotFlags(Self::DECIDED | if escape { Self::ESCAPE } else { 0 } | if end_at { Self::END_AT } else { 0 })
    }
    pub fn escape(self) -> bool {
        self.0 & Self::ESCAPE != 0
    }
    pub fn end_at(self) -> bool {
        self.0 & Self::END_AT != 0
    }
}

/// The flags of slot `s`: the stored ones, else decided now (stored once the run's lists are set; before that they are
/// decided again at each call, which then matches nothing).
fn slot_flags(s: &Slot, dll: &str, name: &str) -> SlotFlags {
    let f = s.flags.load(Ordering::Relaxed);
    if f != 0 {
        return SlotFlags(f);
    }
    let v = SlotFlags::new(crate::run::is_escape_point(dll, name), crate::run::is_end_at(dll, name));
    if crate::run::hook_lists_ready() {
        s.flags.store(v.0, Ordering::Relaxed);
    }
    v
}

struct Table {
    slots: &'static [Slot],
    code: usize,
}

static TABLE: OnceLock<Table> = OnceLock::new();
static NEXT: AtomicUsize = AtomicUsize::new(0);
static ALLOC_LOCK: Mutex<()> = Mutex::new(());

fn table() -> &'static Table {
    TABLE.get_or_init(|| {
        let mut v = Vec::with_capacity(MAX_SLOTS);
        for _ in 0..MAX_SLOTS {
            v.push(Slot {
                dll: OnceLock::new(),
                name: OnceLock::new(),
                imp: AtomicUsize::new(0),
                count: AtomicU64::new(0),
                first_ret: AtomicU64::new(0),
                flags: AtomicU8::new(0),
            });
        }
        let slots: &'static [Slot] = Box::leak(v.into_boxed_slice());
        let code = crate::mem::low_mmap(MAX_SLOTS * THUNK_SIZE, libc::PROT_READ | libc::PROT_WRITE | libc::PROT_EXEC);
        assert!(code != 0, "thunk mmap failed");
        Table { slots, code }
    })
}

thread_local! {
    /// Return address of the latest import call on this thread (the exe caller of the running stub).
    pub static LAST_RET: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

pub fn caller() -> u64 {
    LAST_RET.with(|c| c.get())
}

/// Called by every thunk. Returns the host implementation address.
unsafe extern "win64" fn note(idx: u64, ret: u64) -> u64 {
    LAST_RET.with(|c| c.set(ret));
    let t = table();
    let s = &t.slots[idx as usize];
    let n = s.count.fetch_add(1, Ordering::Relaxed);
    let imp = s.imp.load(Ordering::Relaxed);
    let dll = s.dll.get().map(String::as_str).unwrap_or("?");
    let name = s.name.get().map(String::as_str).unwrap_or("?");
    if n == 0 {
        s.first_ret.store(ret, Ordering::Relaxed);
    }
    if TRACE.load(Ordering::Relaxed) && !matches!(name, "QueryPerformanceCounter" | "GetCurrentThreadId" | "TlsGetValue" | "GetLastError" | "SetLastError") {
        hlog!("trace: tid {} {dll}!{name} from {ret:#x}", crate::teb::gettid());
    }
    let flags = slot_flags(s, dll, name);
    if imp == 0 && !flags.escape() {
        if flags.end_at() {
            crate::k32::maybe_end_thread(dll, name);
        }
        hlog!("TRAP: unimplemented import {dll}!{name} called from {ret:#x}; aborting");
        crate::run::abort_run(3);
    }
    if flags.end_at() {
        crate::k32::maybe_end_thread(dll, name);
    }
    if flags.escape() {
        hlog!("escape point reached: {dll}!{name} called from {ret:#x}");
        crate::k32::escape(0xE5C);
    }
    if n == 0 && crate::run::verbose_stubs() {
        hlog!("stub: first call {dll}!{name} from {ret:#x}");
    }
    imp as u64
}

/// Make a thunk for (dll, name) with implementation `imp` (0 = trap). Returns the thunk address.
pub fn make(dll: &str, name: &str, imp: usize) -> usize {
    let t = table();
    let _g = ALLOC_LOCK.lock().unwrap();
    let idx = NEXT.fetch_add(1, Ordering::Relaxed);
    assert!(idx < MAX_SLOTS, "out of thunk slots");
    let s = &t.slots[idx];
    let _ = s.dll.set(dll.to_string());
    let _ = s.name.set(name.to_string());
    s.imp.store(imp, Ordering::Relaxed);
    let at = t.code + idx * THUNK_SIZE;
    let mut c: Vec<u8> = Vec::with_capacity(THUNK_SIZE);
    c.extend_from_slice(&[0x51, 0x52, 0x41, 0x50, 0x41, 0x51]); // push rcx, rdx, r8, r9
    c.extend_from_slice(&[0x48, 0x83, 0xEC, 0x28]); // sub rsp, 0x28
    c.push(0xB9); // mov ecx, imm32
    c.extend_from_slice(&(idx as u32).to_le_bytes());
    c.extend_from_slice(&[0x48, 0x8B, 0x54, 0x24, 0x48]); // mov rdx, [rsp+0x48]
    c.extend_from_slice(&[0x48, 0xB8]); // movabs rax, note
    c.extend_from_slice(&(note as *const () as usize as u64).to_le_bytes());
    c.extend_from_slice(&[0xFF, 0xD0]); // call rax
    c.extend_from_slice(&[0x48, 0x83, 0xC4, 0x28]); // add rsp, 0x28
    c.extend_from_slice(&[0x41, 0x59, 0x41, 0x58, 0x5A, 0x59]); // pop r9, r8, rdx, rcx
    c.extend_from_slice(&[0xFF, 0xE0]); // jmp rax
    assert!(c.len() <= THUNK_SIZE);
    c.resize(THUNK_SIZE, 0xCC);
    unsafe { std::ptr::copy_nonoverlapping(c.as_ptr(), at as *mut u8, THUNK_SIZE) };
    at
}

/// Report of every import that was called: (dll, name, calls, implemented, first return address).
pub fn report() -> Vec<(String, String, u64, bool, u64)> {
    let Some(t) = TABLE.get() else { return vec![] };
    let n = NEXT.load(Ordering::Relaxed);
    let mut out = Vec::new();
    for s in &t.slots[..n] {
        let c = s.count.load(Ordering::Relaxed);
        if c > 0 {
            out.push((
                s.dll.get().cloned().unwrap_or_default(),
                s.name.get().cloned().unwrap_or_default(),
                c,
                s.imp.load(Ordering::Relaxed) != 0,
                s.first_ret.load(Ordering::Relaxed),
            ));
        }
    }
    out.sort();
    out.dedup_by(|a, b| {
        if a.0 == b.0 && a.1 == b.1 {
            b.2 += a.2;
            true
        } else {
            false
        }
    });
    out
}
