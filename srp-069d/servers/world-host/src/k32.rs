//! KERNEL32 host stubs (Win64 ABI). Only functions the exe actually reaches need to be real;
//! everything not listed in `table()` traps with DLL!name when called.
#![allow(non_snake_case, clippy::too_many_arguments)]

use crate::files;
use crate::handles::{self, Obj};
use crate::hlog;
use crate::mem;
use crate::strings::*;
use crate::teb;
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

type A = u64;
pub const FAKE_K32: u64 = 0x7FF0_0000_1000;
/// Handle of the host's empty dbghelp: the engine's error-report stack dump (0x2000D2680) calls StackWalk64 and the
/// Sym* functions through pointers it got with GetProcAddress, without a null check.
pub const FAKE_DBGHELP: u64 = 0x7FF0_0000_2000;
/// --no-xaudio: CoCreateInstance of XAudio2 fails (the null XAudio2 is the default).
pub static NO_XAUDIO: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub const INVALID_HANDLE: u64 = u64::MAX;

// exe code passes unaligned pointers at times: all accesses are unaligned-safe
unsafe fn w16(p: A, v: u16) {
    unsafe { (p as *mut u16).write_unaligned(v) }
}
unsafe fn w32(p: A, v: u32) {
    unsafe { (p as *mut u32).write_unaligned(v) }
}
unsafe fn w64(p: A, v: u64) {
    unsafe { (p as *mut u64).write_unaligned(v) }
}
unsafe fn r32(p: A) -> u32 {
    unsafe { (p as *const u32).read_unaligned() }
}
unsafe fn r64(p: A) -> u64 {
    unsafe { (p as *const u64).read_unaligned() }
}
unsafe fn zero(p: A, n: usize) {
    unsafe { std::ptr::write_bytes(p as *mut u8, 0, n) }
}
fn err(e: u32) {
    teb::set_last_error(e);
}

// ------------------------------------------------------------------ process configuration

pub struct ProcCfg {
    pub exe_win: String,
    pub cmdline: String,
    pub cores: u32,
}
pub static PROC: OnceLock<ProcCfg> = OnceLock::new();
fn cmdline_w() -> u64 {
    static P: OnceLock<u64> = OnceLock::new();
    *P.get_or_init(|| leak_wstr(&PROC.get().unwrap().cmdline))
}
fn cmdline_a() -> u64 {
    static P: OnceLock<u64> = OnceLock::new();
    *P.get_or_init(|| leak_cstr(&PROC.get().unwrap().cmdline))
}
static T0: OnceLock<Instant> = OnceLock::new();
/// `HOST_CLOCK_OFFSET_S=<seconds>` (diagnosis): the exe's monotonic clock (QueryPerformanceCounter, GetTickCount)
/// reads that much more, as in a process that has run that long. A zygote forked from the base process inherits the
/// base's clock, so its level loads at the base's uptime instead of a few seconds after start; this lets a one-shot
/// run show whether a large clock changes a level load or a match.
static CLOCK_OFFSET: OnceLock<Duration> = OnceLock::new();
fn mono() -> Duration {
    let off = *CLOCK_OFFSET.get_or_init(|| {
        let d = clock_offset(std::env::var("HOST_CLOCK_OFFSET_S").ok().as_deref());
        if !d.is_zero() {
            hlog!("HOST_CLOCK_OFFSET_S: the exe's monotonic clock starts at {} s", d.as_secs());
        }
        d
    });
    T0.get_or_init(Instant::now).elapsed() + off
}

/// The `HOST_CLOCK_OFFSET_S` value: whole seconds, anything else 0.
pub fn clock_offset(v: Option<&str>) -> Duration {
    v.and_then(|s| s.trim().parse::<u64>().ok()).map(Duration::from_secs).unwrap_or_default()
}

// ------------------------------------------------------------------ process / module

unsafe extern "win64" fn GetCommandLineW() -> A {
    cmdline_w()
}
unsafe extern "win64" fn GetCommandLineA() -> A {
    cmdline_a()
}
unsafe extern "win64" fn GetModuleHandleW(name: A) -> A {
    if name == 0 {
        return teb::IMAGE_BASE.load(Ordering::Relaxed);
    }
    module_by_name(&unsafe { wstr(name) })
}
unsafe extern "win64" fn GetModuleHandleA(name: A) -> A {
    if name == 0 {
        return teb::IMAGE_BASE.load(Ordering::Relaxed);
    }
    module_by_name(&unsafe { cstr(name) })
}
fn module_by_name(n: &str) -> A {
    let l = n.to_ascii_lowercase();
    let base = l.rsplit(['\\', '/']).next().unwrap_or(&l).trim_end_matches(".dll").to_string();
    let r = if base == "kernel32" || base == "kernelbase" || base == "ntdll" { FAKE_K32 } else { 0 };
    hlog!("GetModuleHandle({n}) -> {r:#x}");
    if r == 0 {
        err(126);
    }
    r
}
unsafe extern "win64" fn GetModuleHandleExW(flags: A, name: A, out: A) -> A {
    let img = teb::IMAGE_BASE.load(Ordering::Relaxed);
    let r = if flags & 4 != 0 {
        // GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS
        if name >= img && name < img + 0x15E38000 { img } else { 0 }
    } else if name == 0 {
        img
    } else {
        module_by_name(&unsafe { wstr(name) })
    };
    if out != 0 {
        unsafe { w64(out, r) };
    }
    (r != 0) as A
}
unsafe extern "win64" fn GetModuleFileNameW(m: A, buf: A, n: A) -> A {
    let img = teb::IMAGE_BASE.load(Ordering::Relaxed);
    if m != 0 && m != img {
        err(126);
        return 0;
    }
    let p = &PROC.get().unwrap().exe_win;
    let (k, ok) = unsafe { put_wstr(buf, n as usize, p) };
    if !ok {
        err(122);
        return n;
    }
    k as A
}
unsafe extern "win64" fn GetModuleFileNameA(m: A, buf: A, n: A) -> A {
    let img = teb::IMAGE_BASE.load(Ordering::Relaxed);
    if m != 0 && m != img {
        err(126);
        return 0;
    }
    let p = &PROC.get().unwrap().exe_win;
    let (k, ok) = unsafe { put_cstr(buf, n as usize, p) };
    if !ok {
        err(122);
        return n;
    }
    k as A
}
fn load_library(n: &str) -> A {
    let l = n.to_ascii_lowercase();
    let base = l.rsplit(['\\', '/']).next().unwrap_or(&l).to_string();
    let r = if base == "kernel32.dll" || base == "kernel32" || base == "kernelbase.dll" {
        FAKE_K32
    } else if base == "dbghelp.dll" || base == "dbghelp" {
        FAKE_DBGHELP
    } else {
        0
    };
    hlog!("LoadLibrary({n}) -> {r:#x}");
    if r == 0 {
        err(126);
    }
    r
}
unsafe extern "win64" fn LoadLibraryA(n: A) -> A {
    load_library(&unsafe { cstr(n) })
}
unsafe extern "win64" fn LoadLibraryW(n: A) -> A {
    load_library(&unsafe { wstr(n) })
}
unsafe extern "win64" fn LoadLibraryExW(n: A, _f: A, _flags: A) -> A {
    load_library(&unsafe { wstr(n) })
}
unsafe extern "win64" fn FreeLibrary(_h: A) -> A {
    1
}
unsafe extern "win64" fn GetProcAddress(h: A, name: A) -> A {
    let nm = if name < 0x10000 { format!("#{name}") } else { unsafe { cstr(name) } };
    if h == FAKE_K32 {
        static CACHE: Mutex<Vec<(String, u64)>> = Mutex::new(Vec::new());
        if let Some((_, a)) = CACHE.lock().unwrap().iter().find(|(n, _)| *n == nm) {
            return *a;
        }
        let imp = table().into_iter().find(|(n, _)| *n == nm).map(|(_, a)| a);
        let r = match imp {
            Some(a) => crate::thunks::make("KERNEL32.dll(dyn)", &nm, a) as u64,
            None => 0,
        };
        hlog!("GetProcAddress(kernel32, {nm}) -> {}", if r != 0 { "host stub" } else { "NULL" });
        CACHE.lock().unwrap().push((nm, r));
        if r == 0 {
            err(127);
        }
        return r;
    }
    if h == FAKE_DBGHELP {
        // every dbghelp function the exe names (StackWalk64, Sym*, UnDecorateSymbolName, MiniDumpWriteDump) fails:
        // StackWalk64 = FALSE ends the walk at once
        let r = crate::thunks::make("dbghelp.dll(dyn)", &nm, dbghelp_fail as *const () as usize) as u64;
        hlog!("GetProcAddress(dbghelp, {nm}) -> host stub returning 0");
        return r;
    }
    hlog!("GetProcAddress({h:#x}, {nm}) -> NULL");
    err(127);
    0
}
unsafe extern "win64" fn dbghelp_fail() -> A {
    0
}
unsafe extern "win64" fn GetCurrentProcess() -> A {
    handles::PSEUDO_PROCESS
}
unsafe extern "win64" fn GetCurrentProcessId() -> A {
    unsafe { libc::getpid() as A }
}
unsafe extern "win64" fn GetCurrentThread() -> A {
    handles::PSEUDO_THREAD
}
thread_local! {
    /// Thread id this thread reports (0 = its own). The host's game-creation thread reports the world thread,
    /// so the cooks the game ctor registers (cook+0x18 = GetCurrentThreadId) are served by the world thread,
    /// as in the client where one thread creates the game and loads levels.
    pub static REPORTED_TID: Cell<u64> = const { Cell::new(0) };
}
/// The game ctor's UI start-up query (0x2001151C0, called at 0x200110083) must stay queued to the parked creation
/// thread: from its issue (first GetCurrentThreadId from 0x2000BC83B in 0x2000BC810) until the ctor continues
/// (GetCurrentThreadId from 0x2001100BA) the creation thread reports its real id.
thread_local! {
    static REAL_WINDOW: Cell<bool> = const { Cell::new(false) };
}
/// The thread id the exe sees on this thread (the reported one if set, else the real one).
#[allow(non_snake_case)]
pub fn GetCurrentThreadIdPub() -> u64 {
    let f = FORCED_TID.with(|t| t.get());
    if f != 0 {
        return f;
    }
    let r = REPORTED_TID.with(|t| t.get());
    if r != 0 { r } else { teb::gettid() }
}
thread_local! {
    /// A fork()ed child's world thread: always report the parent's world thread id (no creation window).
    pub static FORCED_TID: Cell<u64> = const { Cell::new(0) };
}
unsafe extern "win64" fn GetCurrentThreadId() -> A {
    let f = FORCED_TID.with(|t| t.get());
    if f != 0 {
        return f;
    }
    let r = REPORTED_TID.with(|t| t.get());
    if r != 0 {
        let c = crate::thunks::caller();
        if c == 0x2000BC83B && !REAL_WINDOW.with(|w| w.get()) {
            REAL_WINDOW.with(|w| w.set(true));
            hlog!("game-creation thread: start-up query issued; real thread id until the ctor continues");
        } else if c == 0x2001100BA && REAL_WINDOW.with(|w| w.get()) {
            REAL_WINDOW.with(|w| w.set(false));
            hlog!("game-creation thread: ctor continues; reporting the world thread id again");
        }
        if !REAL_WINDOW.with(|w| w.get()) {
            return r;
        }
    }
    teb::gettid()
}
unsafe extern "win64" fn ExitProcess(code: A) -> A {
    hlog!("ExitProcess({code})");
    crate::run::abort_run(code as i32)
}
unsafe extern "win64" fn TerminateProcess(h: A, code: A) -> A {
    hlog!("TerminateProcess({h:#x}, {code})");
    crate::run::abort_run(code as i32)
}
unsafe extern "win64" fn GetStartupInfoW(p: A) -> A {
    unsafe {
        zero(p, 104);
        w32(p, 104);
    }
    0
}
unsafe extern "win64" fn GetEnvironmentStringsW() -> A {
    static P: OnceLock<u64> = OnceLock::new();
    *P.get_or_init(|| unsafe { mem::heap_alloc(8, true) } as u64)
}
unsafe extern "win64" fn FreeEnvironmentStringsW(_p: A) -> A {
    1
}
unsafe extern "win64" fn GetEnvironmentVariableA(n: A, _b: A, _s: A) -> A {
    hlog!("GetEnvironmentVariableA({}) -> not found", unsafe { cstr(n) });
    err(203);
    0
}
unsafe extern "win64" fn GetEnvironmentVariableW(n: A, _b: A, _s: A) -> A {
    hlog!("GetEnvironmentVariableW({}) -> not found", unsafe { wstr(n) });
    err(203);
    0
}
unsafe extern "win64" fn SetEnvironmentVariableW(_n: A, _v: A) -> A {
    1
}
unsafe extern "win64" fn GetCurrentDirectoryW(n: A, buf: A) -> A {
    let cwd = &files::CFG.get().unwrap().cwd_win;
    let (k, ok) = unsafe { put_wstr(buf, n as usize, cwd) };
    if ok { k as A } else { k as A }
}
unsafe extern "win64" fn GetSystemDirectoryW(buf: A, n: A) -> A {
    let (k, _) = unsafe { put_wstr(buf, n as usize, "C:\\Windows\\system32") };
    k as A
}
unsafe extern "win64" fn GetVersion() -> A {
    0x23F0_0206 // 6.2 build 9200, what an unmanifested app sees on Windows 10
}
unsafe extern "win64" fn VerifyVersionInfoW(_vi: A, _mask: A, _cond: A) -> A {
    1
}
unsafe extern "win64" fn VerSetConditionMask(mask: A, typ: A, cond: A) -> A {
    mask | ((cond & 7) << ((typ.trailing_zeros() as u64) * 3))
}
unsafe extern "win64" fn IsProcessorFeaturePresent(f: A) -> A {
    let v = match f {
        2 | 3 | 6 | 8 | 9 | 10 | 12 | 13 | 14 | 17 | 20 | 23 => true,
        36 => std::arch::is_x86_feature_detected!("ssse3"),
        37 => std::arch::is_x86_feature_detected!("sse4.1"),
        38 => std::arch::is_x86_feature_detected!("sse4.2"),
        39 => std::arch::is_x86_feature_detected!("avx"),
        40 => std::arch::is_x86_feature_detected!("avx2"),
        _ => false,
    };
    v as A
}
unsafe extern "win64" fn GetSystemInfo(p: A) -> A {
    let n = PROC.get().map(|c| c.cores).unwrap_or(2);
    unsafe {
        zero(p, 48);
        w16(p, 9);
        w32(p + 4, 4096);
        w64(p + 8, 0x10000);
        w64(p + 16, 0x7FFF_FFFE_FFFF);
        w64(p + 24, (1u64 << n) - 1);
        w32(p + 32, n);
        w32(p + 36, 8664);
        w32(p + 40, 0x10000);
        w16(p + 44, 6);
    }
    0
}
unsafe extern "win64" fn SetPriorityClass(_h: A, _c: A) -> A {
    1
}

// ------------------------------------------------------------------ memory

unsafe extern "win64" fn VirtualAlloc(addr: A, size: A, typ: A, prot: A) -> A {
    static RESERVED: AtomicU64 = AtomicU64::new(0);
    static COMMITTED: AtomicU64 = AtomicU64::new(0);
    let r = unsafe { mem::virtual_alloc(addr as usize, size as usize, typ as u32, prot as u32) } as A;
    let kind = match (typ & 0x2000 != 0, typ & 0x1000 != 0) {
        (true, true) => "reserve+commit",
        (true, false) => "reserve",
        (false, true) => "commit",
        _ => "other",
    };
    if r != 0 && typ & 0x2000 != 0 {
        RESERVED.fetch_add(size, Ordering::Relaxed);
    }
    if r != 0 && typ & 0x1000 != 0 {
        COMMITTED.fetch_add(size, Ordering::Relaxed);
    }
    {
        hlog!(
            "VirtualAlloc({addr:#x}, {:.2} MB, {kind}, prot {prot:#x}) from {:#x} -> {r:#x}; totals reserved {:.2} MB, committed {:.2} MB",
            size as f64 / 1048576.0,
            crate::thunks::caller(),
            RESERVED.load(Ordering::Relaxed) as f64 / 1048576.0,
            COMMITTED.load(Ordering::Relaxed) as f64 / 1048576.0
        );
    }
    if r == 0 {
        err(8);
    }
    r
}
unsafe extern "win64" fn VirtualFree(addr: A, size: A, typ: A) -> A {
    unsafe { mem::virtual_free(addr as usize, size as usize, typ as u32) as A }
}
unsafe extern "win64" fn VirtualQuery(addr: A, out: A, len: A) -> A {
    match mem::virtual_query(addr as usize) {
        Some(m) if len >= 48 => {
            unsafe { std::ptr::write(out as *mut mem::Mbi, m) };
            48
        }
        _ => {
            err(87);
            0
        }
    }
}
unsafe extern "win64" fn GetProcessHeap() -> A {
    teb::FAKE_PROCESS_HEAP
}
unsafe extern "win64" fn HeapAlloc(_h: A, flags: A, size: A) -> A {
    unsafe { mem::heap_alloc(size as usize, flags & 8 != 0) as A }
}
unsafe extern "win64" fn HeapReAlloc(_h: A, flags: A, p: A, size: A) -> A {
    unsafe { mem::heap_realloc(p as usize, size as usize, flags & 8 != 0, flags & 0x10 != 0) as A }
}
unsafe extern "win64" fn HeapFree(_h: A, _f: A, p: A) -> A {
    unsafe { mem::heap_free(p as usize) as A }
}
unsafe extern "win64" fn HeapSize(_h: A, _f: A, p: A) -> A {
    unsafe { mem::heap_size(p as usize) as A }
}
unsafe extern "win64" fn HeapWalk(_h: A, _e: A) -> A {
    err(259);
    0
}
unsafe extern "win64" fn HeapValidate(_h: A, _f: A, _p: A) -> A {
    1
}
unsafe extern "win64" fn GlobalAlloc(flags: A, size: A) -> A {
    unsafe { mem::heap_alloc(size as usize, flags & 0x40 != 0) as A }
}
unsafe extern "win64" fn GlobalFree(p: A) -> A {
    unsafe { mem::heap_free(p as usize) };
    0
}
unsafe extern "win64" fn GlobalLock(p: A) -> A {
    p
}
unsafe extern "win64" fn GlobalUnlock(_p: A) -> A {
    1
}
unsafe extern "win64" fn LocalAlloc(flags: A, size: A) -> A {
    unsafe { mem::heap_alloc(size as usize, flags & 0x40 != 0) as A }
}
unsafe extern "win64" fn LocalFree(p: A) -> A {
    unsafe { mem::heap_free(p as usize) };
    0
}
const MB: u64 = 1 << 20;
unsafe extern "win64" fn GlobalMemoryStatusEx(p: A) -> A {
    unsafe {
        w32(p + 4, 50);
        w64(p + 8, 16384 * MB);
        w64(p + 16, 8192 * MB);
        w64(p + 24, 24576 * MB);
        w64(p + 32, 16384 * MB);
        w64(p + 40, 0x7FFF_FFFE_0000);
        w64(p + 48, 0x7FFF_0000_0000);
        w64(p + 56, 0);
    }
    1
}
unsafe extern "win64" fn GlobalMemoryStatus(p: A) -> A {
    unsafe {
        w32(p + 4, 50);
        w64(p + 8, 16384 * MB);
        w64(p + 16, 8192 * MB);
        w64(p + 24, 24576 * MB);
        w64(p + 32, 16384 * MB);
        w64(p + 40, 0x7FFF_FFFE_0000);
        w64(p + 48, 0x7FFF_0000_0000);
    }
    0
}

// ------------------------------------------------------------------ TLS

static TLS_USED: Mutex<Vec<bool>> = Mutex::new(Vec::new());
unsafe extern "win64" fn TlsAlloc() -> A {
    let mut u = TLS_USED.lock().unwrap();
    if u.is_empty() {
        u.resize(teb::TLS_SLOTS + teb::TLS_EXPANSION, false);
    }
    match u.iter().position(|&x| !x) {
        Some(i) => {
            u[i] = true;
            i as A
        }
        None => 0xFFFF_FFFF,
    }
}
unsafe extern "win64" fn TlsFree(i: A) -> A {
    let mut u = TLS_USED.lock().unwrap();
    if (i as usize) < u.len() {
        u[i as usize] = false;
    }
    1
}
unsafe extern "win64" fn TlsGetValue(i: A) -> A {
    let v = teb::tls_get(i as usize);
    err(0);
    v
}
unsafe extern "win64" fn TlsSetValue(i: A, v: A) -> A {
    teb::tls_set(i as usize, v);
    1
}

// ------------------------------------------------------------------ errors / debug

unsafe extern "win64" fn GetLastError() -> A {
    teb::last_error() as A
}
unsafe extern "win64" fn SetLastError(e: A) -> A {
    err(e as u32);
    0
}
unsafe extern "win64" fn OutputDebugStringA(s: A) -> A {
    let t = unsafe { cstr(s) };
    for l in t.trim_end_matches(['\n', '\r']).split('\n') {
        crate::log::line(&format!("dbg: {}", l.trim_end_matches('\r')));
    }
    0
}
/// 1 by default: the engine's log goes to OutputDebugStringA only when a debugger is present (0x2000944FE), and the
/// reference client logs were taken under a debugger, so 1 reproduces their conditions. `--no-debugger` answers 0.
pub static DEBUGGER: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
unsafe extern "win64" fn IsDebuggerPresent() -> A {
    DEBUGGER.load(Ordering::Relaxed) as A
}
unsafe extern "win64" fn SetUnhandledExceptionFilter(_f: A) -> A {
    0
}
/// Used by the engine's error-report stack dump (0x2000D272E) after a non-fatal error such as "Sound initialization
/// FAILED": the caller zeroed the CONTEXT and set its flags; the host leaves it zeroed (no stack walk follows,
/// dbghelp is not loadable).
unsafe extern "win64" fn RtlCaptureContext(_ctx: A) -> A {
    0
}
unsafe extern "win64" fn UnhandledExceptionFilter(_p: A) -> A {
    0
}
unsafe extern "win64" fn RaiseException(code: A, flags: A, n: A, args: A) -> A {
    let code = code as u32;
    if code == 0x406D_1388 {
        // MSVC thread naming: THREADNAME_INFO {type 0x1000, name, tid, flags}
        if n >= 2 && args != 0 {
            let name = unsafe { cstr(r64(args + 8)) };
            let tid = unsafe { r32(args + 16) };
            let me = teb::gettid() as u32;
            hlog!("thread name: {name} (tid field {tid:#x})");
            if tid == 0xFFFF_FFFF || tid == me {
                THREAD_NAME.with(|t| *t.borrow_mut() = name.clone());
                if let Some(e) = EXE_THREADS.lock().unwrap().iter_mut().find(|e| e.0 == me as u64) {
                    e.3 = name.clone();
                }
            }
        }
        return 0;
    }
    hlog!("RaiseException({code:#x}, flags {flags:#x}, {n} args) - no unwinder in the host; aborting");
    crate::run::abort_run(6)
}
unsafe extern "win64" fn EncodePointer(p: A) -> A {
    p
}
unsafe extern "win64" fn DecodePointer(p: A) -> A {
    p
}
unsafe extern "win64" fn FormatMessageA(_f: A, _s: A, _id: A, _l: A, _b: A, _n: A, _a: A) -> A {
    0
}
unsafe extern "win64" fn GetConsoleWindow() -> A {
    0
}
unsafe extern "win64" fn AttachConsole(_p: A) -> A {
    0
}
unsafe extern "win64" fn AllocConsole() -> A {
    0
}
unsafe extern "win64" fn GetStdHandle(_n: A) -> A {
    0
}
unsafe extern "win64" fn SetStdHandle(_n: A, _h: A) -> A {
    1
}
unsafe extern "win64" fn SetConsoleCtrlHandler(_f: A, _a: A) -> A {
    1
}
unsafe extern "win64" fn GetConsoleCP() -> A {
    437
}
unsafe extern "win64" fn GetConsoleMode(_h: A, _m: A) -> A {
    0
}

// ------------------------------------------------------------------ strings / locale

unsafe extern "win64" fn MultiByteToWideChar(cp: A, _fl: A, src: A, n: A, dst: A, cap: A) -> A {
    let s = unsafe { cbytes(src, n as i32 as i64) };
    let mut w = mb_to_wide(cp as u32, s);
    if n as i32 == -1 {
        w.push(0);
    }
    if cap as i32 == 0 {
        return w.len() as A;
    }
    if w.len() > cap as usize {
        err(122);
        return 0;
    }
    unsafe { std::ptr::copy_nonoverlapping(w.as_ptr(), dst as *mut u16, w.len()) };
    w.len() as A
}
unsafe extern "win64" fn WideCharToMultiByte(cp: A, _fl: A, src: A, n: A, dst: A, cap: A, _def: A, used: A) -> A {
    let s = unsafe { wslice(src, n as i32 as i64) };
    let (mut b, u) = wide_to_mb(cp as u32, s);
    if n as i32 == -1 {
        b.push(0);
    }
    if used != 0 {
        unsafe { w32(used, u as u32) };
    }
    if cap as i32 == 0 {
        return b.len() as A;
    }
    if b.len() > cap as usize {
        err(122);
        return 0;
    }
    unsafe { std::ptr::copy_nonoverlapping(b.as_ptr(), dst as *mut u8, b.len()) };
    b.len() as A
}
unsafe extern "win64" fn LCMapStringW(_lcid: A, flags: A, src: A, n: A, dst: A, cap: A) -> A {
    let s = unsafe { wslice(src, n as i32 as i64) };
    let mut v: Vec<u16> = s
        .iter()
        .map(|&c| {
            if flags & 0x100 != 0 {
                wlower(c)
            } else if flags & 0x200 != 0 {
                wupper(c)
            } else {
                c
            }
        })
        .collect();
    if flags & 0x400 != 0 {
        hlog!("LCMapStringW: LCMAP_SORTKEY not supported");
    }
    if n as i32 == -1 {
        v.push(0);
    }
    if cap as i32 == 0 {
        return v.len() as A;
    }
    if v.len() > cap as usize {
        err(122);
        return 0;
    }
    unsafe { std::ptr::copy_nonoverlapping(v.as_ptr(), dst as *mut u16, v.len()) };
    v.len() as A
}
unsafe extern "win64" fn LCMapStringA(_lcid: A, flags: A, src: A, n: A, dst: A, cap: A) -> A {
    let s = unsafe { cbytes(src, n as i32 as i64) };
    let mut v: Vec<u8> = s
        .iter()
        .map(|&c| {
            if flags & 0x100 != 0 {
                c.to_ascii_lowercase()
            } else if flags & 0x200 != 0 {
                c.to_ascii_uppercase()
            } else {
                c
            }
        })
        .collect();
    if n as i32 == -1 {
        v.push(0);
    }
    if cap as i32 == 0 {
        return v.len() as A;
    }
    if v.len() > cap as usize {
        err(122);
        return 0;
    }
    unsafe { std::ptr::copy_nonoverlapping(v.as_ptr(), dst as *mut u8, v.len()) };
    v.len() as A
}
unsafe extern "win64" fn GetStringTypeW(typ: A, src: A, n: A, out: A) -> A {
    let s = unsafe { wslice(src, n as i32 as i64) };
    for (i, &c) in s.iter().enumerate() {
        let v = if typ == 1 { ctype1(c) } else { 0 };
        unsafe { w16(out + 2 * i as u64, v) };
    }
    1
}
unsafe extern "win64" fn GetStringTypeExA(_lcid: A, typ: A, src: A, n: A, out: A) -> A {
    let s = unsafe { cbytes(src, n as i32 as i64) };
    for (i, &c) in s.iter().enumerate() {
        let v = if typ == 1 { ctype1(cp1252_to_u16(c)) } else { 0 };
        unsafe { w16(out + 2 * i as u64, v) };
    }
    1
}
unsafe extern "win64" fn CompareStringW(_lcid: A, flags: A, s1: A, n1: A, s2: A, n2: A) -> A {
    let a = unsafe { wslice(s1, n1 as i32 as i64) };
    let b = unsafe { wslice(s2, n2 as i32 as i64) };
    let f = |c: u16| if flags & 1 != 0 { wlower(c) } else { c };
    let x: Vec<u16> = a.iter().map(|&c| f(c)).collect();
    let y: Vec<u16> = b.iter().map(|&c| f(c)).collect();
    match x.cmp(&y) {
        std::cmp::Ordering::Less => 1,
        std::cmp::Ordering::Equal => 2,
        std::cmp::Ordering::Greater => 3,
    }
}
unsafe extern "win64" fn FoldStringW(_f: A, src: A, n: A, dst: A, cap: A) -> A {
    unsafe { LCMapStringW(0, 0, src, n, dst, cap) }
}
unsafe extern "win64" fn GetCPInfo(cp: A, p: A) -> A {
    unsafe {
        zero(p, 20);
        w32(p, if cp == 65001 { 4 } else { 1 });
        *((p + 4) as *mut u8) = b'?';
    }
    1
}
unsafe extern "win64" fn GetACP() -> A {
    1252
}
unsafe extern "win64" fn GetOEMCP() -> A {
    437
}
unsafe extern "win64" fn IsValidCodePage(cp: A) -> A {
    matches!(cp, 437 | 1252 | 65001) as A
}
unsafe extern "win64" fn IsValidLocale(_l: A, _f: A) -> A {
    1
}
unsafe extern "win64" fn GetUserDefaultLCID() -> A {
    0x409
}
unsafe extern "win64" fn GetLocaleInfoW(lcid: A, typ: A, buf: A, n: A) -> A {
    let t = typ & 0x0FFF_FFFF;
    let s = match t {
        0x1004 => "1252", // LOCALE_IDEFAULTANSICODEPAGE
        0x000B => "437",  // LOCALE_IDEFAULTCODEPAGE
        0x0059 => "en",   // LOCALE_SISO639LANGNAME
        0x005A => "US",   // LOCALE_SISO3166CTRYNAME
        0x0001 => "0409",
        0x0003 => "ENU",           // LOCALE_SABBREVLANGNAME
        0x0007 => "USA",           // LOCALE_SABBREVCTRYNAME
        0x1001 => "English",       // LOCALE_SENGLANGUAGE
        0x1002 => "United States", // LOCALE_SENGCOUNTRY
        // en-US LC_TIME / LC_NUMERIC / LC_MONETARY answers the UCRT asks for
        0x2A => "Monday", 0x2B => "Tuesday", 0x2C => "Wednesday", 0x2D => "Thursday", 0x2E => "Friday",
        0x2F => "Saturday", 0x30 => "Sunday",
        0x31 => "Mon", 0x32 => "Tue", 0x33 => "Wed", 0x34 => "Thu", 0x35 => "Fri", 0x36 => "Sat", 0x37 => "Sun",
        0x38 => "January", 0x39 => "February", 0x3A => "March", 0x3B => "April", 0x3C => "May", 0x3D => "June",
        0x3E => "July", 0x3F => "August", 0x40 => "September", 0x41 => "October", 0x42 => "November", 0x43 => "December",
        0x44 => "Jan", 0x45 => "Feb", 0x46 => "Mar", 0x47 => "Apr", 0x48 => "May", 0x49 => "Jun",
        0x4A => "Jul", 0x4B => "Aug", 0x4C => "Sep", 0x4D => "Oct", 0x4E => "Nov", 0x4F => "Dec",
        0x28 => "AM", 0x29 => "PM", 0x1F => "M/d/yyyy", 0x20 => "dddd, MMMM d, yyyy", 0x1003 => "h:mm:ss tt",
        0x1009 => "1", 0x0E => ".", 0x0F => ",", 0x10 => "3;0", 0x14 => "$", 0x15 => "USD", 0x16 => ".",
        0x17 => ",", 0x18 => "3;0", 0x19 => "2", 0x1A => "2", 0x1B => "0", 0x1C => "0", 0x50 => "", 0x51 => "-",
        0x52 => "3", 0x53 => "0", 0x54 => "1", 0x55 => "0", 0x56 => "1", 0x57 => "0", 0x1014 => "2",
        _ => {
            hlog!("GetLocaleInfoW({lcid:#x}, {typ:#x}) unknown -> 0");
            err(87);
            return 0;
        }
    };
    if typ & 0x2000_0000 != 0 {
        // LOCALE_RETURN_NUMBER
        if n >= 2 {
            unsafe { w32(buf, s.parse::<u32>().unwrap_or(0)) };
        }
        return 2;
    }
    let (k, ok) = unsafe { put_wstr(buf, n as usize, s) };
    if n == 0 {
        return k as A;
    }
    if ok { (k + 1) as A } else { 0 }
}

// ------------------------------------------------------------------ time

unsafe extern "win64" fn QueryPerformanceFrequency(p: A) -> A {
    unsafe { w64(p, 10_000_000) };
    1
}
unsafe extern "win64" fn QueryPerformanceCounter(p: A) -> A {
    unsafe { w64(p, (mono().as_nanos() / 100) as u64 + 10_000_000) };
    1
}
unsafe extern "win64" fn GetTickCount() -> A {
    (mono().as_millis() as u64 + 1_000_000) & 0xFFFF_FFFF
}
fn now_filetime() -> u64 {
    let d = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap();
    files::unix_to_filetime(d.as_secs() as i64, d.subsec_nanos() as i64)
}
unsafe extern "win64" fn GetSystemTimeAsFileTime(p: A) -> A {
    unsafe { w64(p, now_filetime()) };
    0
}
unsafe fn put_systemtime(p: A, ft: u64) {
    let secs = (ft / 10_000_000) as i64 - 11_644_473_600;
    let ms = ((ft / 10_000) % 1000) as u16;
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let t = secs as libc::time_t;
    unsafe {
        libc::gmtime_r(&t, &mut tm);
        w16(p, (tm.tm_year + 1900) as u16);
        w16(p + 2, (tm.tm_mon + 1) as u16);
        w16(p + 4, tm.tm_wday as u16);
        w16(p + 6, tm.tm_mday as u16);
        w16(p + 8, tm.tm_hour as u16);
        w16(p + 10, tm.tm_min as u16);
        w16(p + 12, tm.tm_sec as u16);
        w16(p + 14, ms);
    }
}
unsafe extern "win64" fn GetLocalTime(p: A) -> A {
    unsafe { put_systemtime(p, now_filetime()) };
    0
}
unsafe extern "win64" fn FileTimeToSystemTime(ft: A, st: A) -> A {
    unsafe { put_systemtime(st, r64(ft)) };
    1
}
unsafe extern "win64" fn SystemTimeToTzSpecificLocalTime(_tz: A, a: A, b: A) -> A {
    unsafe { std::ptr::copy_nonoverlapping(a as *const u8, b as *mut u8, 16) };
    1
}
unsafe extern "win64" fn GetTimeZoneInformation(p: A) -> A {
    unsafe { zero(p, 172) };
    0
}

// ------------------------------------------------------------------ critical sections
// In-place: +0x08 LockCount (kept -1 when free), +0x0C RecursionCount, +0x10 OwningThread (atomic owner tid).

unsafe fn cs_owner(cs: A) -> &'static AtomicU64 {
    unsafe { &*((cs + 0x10) as *const AtomicU64) }
}
unsafe extern "win64" fn InitializeCriticalSection(cs: A) -> A {
    unsafe {
        zero(cs, 40);
        w32(cs + 8, 0xFFFF_FFFF);
    }
    0
}
unsafe extern "win64" fn InitializeCriticalSectionAndSpinCount(cs: A, _s: A) -> A {
    unsafe { InitializeCriticalSection(cs) };
    1
}
unsafe extern "win64" fn InitializeCriticalSectionEx(cs: A, _s: A, _f: A) -> A {
    unsafe { InitializeCriticalSection(cs) };
    1
}
unsafe extern "win64" fn DeleteCriticalSection(_cs: A) -> A {
    0
}
unsafe extern "win64" fn EnterCriticalSection(cs: A) -> A {
    let me = teb::gettid();
    let o = unsafe { cs_owner(cs) };
    if o.load(Ordering::Acquire) == me {
        unsafe { w32(cs + 12, r32(cs + 12) + 1) };
        return 0;
    }
    let mut spins = 0u32;
    while o.compare_exchange_weak(0, me, Ordering::Acquire, Ordering::Relaxed).is_err() {
        spins += 1;
        if spins < 64 {
            std::hint::spin_loop();
        } else if spins < 256 {
            std::thread::yield_now();
        } else {
            std::thread::sleep(Duration::from_micros(50));
        }
    }
    unsafe {
        w32(cs + 12, 1);
        w32(cs + 8, 0xFFFF_FFFE);
    }
    HELD_CS.with(|h| h.set(h.get() + 1));
    0
}
unsafe extern "win64" fn TryEnterCriticalSection(cs: A) -> A {
    let me = teb::gettid();
    let o = unsafe { cs_owner(cs) };
    if o.load(Ordering::Acquire) == me {
        unsafe { w32(cs + 12, r32(cs + 12) + 1) };
        return 1;
    }
    if o.compare_exchange(0, me, Ordering::Acquire, Ordering::Relaxed).is_ok() {
        unsafe {
            w32(cs + 12, 1);
            w32(cs + 8, 0xFFFF_FFFE);
        }
        HELD_CS.with(|h| h.set(h.get() + 1));
        return 1;
    }
    0
}
unsafe extern "win64" fn LeaveCriticalSection(cs: A) -> A {
    unsafe {
        let r = r32(cs + 12);
        if r <= 1 {
            w32(cs + 12, 0);
            w32(cs + 8, 0xFFFF_FFFF);
            cs_owner(cs).store(0, Ordering::Release);
            HELD_CS.with(|h| h.set(h.get() - 1));
        } else {
            w32(cs + 12, r - 1);
        }
    }
    0
}
/// The host's own TryEnterCriticalSection on an exe critical section, for host code that reads exe state under the
/// exe's lock (`registry`): the same call an exe thread makes, so the section's bytes are those of a free section again
/// after [`critical_section_leave`].
pub fn critical_section_try_enter(cs: u64) -> bool {
    unsafe { TryEnterCriticalSection(cs) != 0 }
}
/// The host's own LeaveCriticalSection, after a successful [`critical_section_try_enter`] on the same thread.
pub fn critical_section_leave(cs: u64) {
    unsafe { LeaveCriticalSection(cs) };
}
static SLIST: Mutex<()> = Mutex::new(());
unsafe extern "win64" fn InitializeSListHead(h: A) -> A {
    unsafe { zero(h, 16) };
    0
}
/// x64 SLIST_HEADER: low qword = depth:16 | sequence:48, high qword = first entry (16-aligned).
/// Under a host lock (the exe imports only the push; pops would need the same layout).
unsafe extern "win64" fn InterlockedPushEntrySList(h: A, e: A) -> A {
    let _g = SLIST.lock().unwrap();
    unsafe {
        let lo = r64(h);
        let first = r64(h + 8) & !0xF;
        w64(e, first);
        let depth = (lo & 0xFFFF) + 1;
        let seq = (lo >> 16) + 1;
        w64(h, (depth & 0xFFFF) | (seq << 16));
        w64(h + 8, e);
        first
    }
}

// ------------------------------------------------------------------ threads / sync

unsafe extern "C" {
    fn wh_call_guarded(f: u64, arg: u64, saved_rsp: *mut u64) -> u64;
    fn wh_call_guarded2(f: u64, arg: u64, saved_rsp: *mut u64, arg2: u64) -> u64;
    fn wh_call_guarded3(f: u64, arg: u64, saved_rsp: *mut u64, arg2: u64, arg3: u64) -> u64;
    fn wh_longjmp_exit(rsp: u64, code: u64) -> !;
}
std::arch::global_asm!(
    ".globl wh_call_guarded",
    "wh_call_guarded:",
    "push rbp",
    "push rbx",
    "push r12",
    "push r13",
    "push r14",
    "push r15",
    "sub rsp, 8",
    "mov [rdx], rsp",
    "mov rcx, rsi",
    "sub rsp, 32",
    "call rdi",
    "add rsp, 32",
    "wh_guard_ret:",
    "add rsp, 8",
    "pop r15",
    "pop r14",
    "pop r13",
    "pop r12",
    "pop rbx",
    "pop rbp",
    "ret",
    ".globl wh_call_guarded2",
    "wh_call_guarded2:",
    "push rbp",
    "push rbx",
    "push r12",
    "push r13",
    "push r14",
    "push r15",
    "sub rsp, 8",
    "mov [rdx], rsp",
    "mov rdx, rcx",
    "mov rcx, rsi",
    "sub rsp, 32",
    "call rdi",
    "add rsp, 32",
    "jmp wh_guard_ret",
    ".globl wh_call_guarded3",
    "wh_call_guarded3:",
    "push rbp",
    "push rbx",
    "push r12",
    "push r13",
    "push r14",
    "push r15",
    "sub rsp, 8",
    "mov [rdx], rsp",
    "mov rdx, rcx",
    "mov rcx, rsi",
    "sub rsp, 32",
    "call rdi",
    "add rsp, 32",
    "jmp wh_guard_ret",
    ".globl wh_longjmp_exit",
    "wh_longjmp_exit:",
    "mov rsp, rdi",
    "mov rax, rsi",
    "jmp wh_guard_ret",
);

thread_local! {
    static GUARD_RSP: Cell<u64> = const { Cell::new(0) };
    /// name the exe gave this thread (MSVC thread-naming exception)
    pub static THREAD_NAME: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

/// Called for every import: a thread of a subsystem the world host does not run (`--end-threads`)
/// ends at its first subsystem import (`--end-at`), i.e. after the spawn handshake (flag set at
/// 0x2000923A2) and before the subsystem does anything.
pub fn maybe_end_thread(dll: &str, name: &str) {
    let tn = THREAD_NAME.with(|t| t.borrow().clone());
    if tn.is_empty() || !crate::run::is_ended_thread(&tn) || !crate::run::is_end_at(dll, name) {
        return;
    }
    hlog!("ending thread {tn} at its first subsystem call {dll}!{name} (from {:#x})", crate::thunks::caller());
    if GUARD_RSP.with(|g| g.get()) == 0 {
        return;
    }
    unsafe { ExitThread(0) };
}

/// Call exe function `f(a1, a2)` on this thread so that `escape()` (or ExitThread) can leave it.
/// Returns Ok(the return value) or Err(code) when it was left through escape().
pub fn call_guarded2(f: u64, a1: u64, a2: u64) -> Result<u64, u64> {
    let mut saved: u64 = 0;
    let prev = GUARD_RSP.with(|g| g.replace(&mut saved as *mut u64 as u64));
    ESCAPED.with(|e| e.set(false));
    let r = unsafe { wh_call_guarded2(f, a1, &mut saved, a2) };
    GUARD_RSP.with(|g| g.set(prev));
    if ESCAPED.with(|e| e.get()) { Err(r) } else { Ok(r) }
}
thread_local! {
    static ESCAPED: Cell<bool> = const { Cell::new(false) };
    /// critical sections currently held by this thread (checked when escaping)
    pub static HELD_CS: Cell<i64> = const { Cell::new(0) };
}
thread_local! {
    /// set while a guarded call may be left on a fault (see `fault_escape`)
    static FAULT_ESCAPE: Cell<bool> = const { Cell::new(false) };
}
/// Call exe function `f(a1)` so that a fault inside it returns here as Err(FAULT_CODE) instead of ending the run (the
/// fault is logged by the crash handler; the exe frames in between are abandoned, nothing is unwound).
pub const FAULT_CODE: u64 = 0xFA17;
/// `call_fault_guarded` with three arguments.
pub fn call_fault_guarded3(f: u64, a1: u64, a2: u64, a3: u64) -> Result<u64, u64> {
    FAULT_ESCAPE.with(|x| x.set(true));
    let mut saved: u64 = 0;
    let prev = GUARD_RSP.with(|g| g.replace(&mut saved as *mut u64 as u64));
    ESCAPED.with(|e| e.set(false));
    let r = unsafe { wh_call_guarded3(f, a1, &mut saved, a2, a3) };
    GUARD_RSP.with(|g| g.set(prev));
    FAULT_ESCAPE.with(|x| x.set(false));
    if ESCAPED.with(|e| e.get()) { Err(r) } else { Ok(r) }
}
pub fn call_fault_guarded(f: u64, a1: u64) -> Result<u64, u64> {
    FAULT_ESCAPE.with(|x| x.set(true));
    let r = call_guarded2(f, a1, 0);
    FAULT_ESCAPE.with(|x| x.set(false));
    r
}
/// For the crash handler: when this thread is inside `call_fault_guarded`, the (instruction pointer, first and second argument register) to resume at so that
/// the guarded call returns Err(FAULT_CODE).
pub fn fault_escape() -> Option<(u64, u64, u64)> {
    if !FAULT_ESCAPE.with(|x| x.get()) {
        return None;
    }
    let p = GUARD_RSP.with(|g| g.get());
    if p == 0 {
        return None;
    }
    FAULT_ESCAPE.with(|x| x.set(false));
    ESCAPED.with(|e| e.set(true));
    Some((wh_longjmp_exit as *const () as u64, unsafe { *(p as *const u64) }, FAULT_CODE))
}
/// Leave the current guarded call (the exe frames in between are abandoned, nothing is unwound).
pub fn escape(code: u64) -> ! {
    let p = GUARD_RSP.with(|g| g.get());
    if p == 0 {
        hlog!("escape requested on a thread without a guard; aborting");
        crate::run::abort_run(7);
    }
    hlog!("escape: leaving the guarded exe call; critical sections held by this thread: {}", HELD_CS.with(|h| h.get()));
    ESCAPED.with(|e| e.set(true));
    let rsp = unsafe { *(p as *const u64) };
    unsafe { wh_longjmp_exit(rsp, code) }
}

/// Every exe thread the host started: (tid, UCRT procedure, copy of the engine's thread start data or 0, name).
pub static EXE_THREADS: std::sync::Mutex<Vec<(u64, u64, u64, String)>> = std::sync::Mutex::new(Vec::new());
/// In a fork()ed child: start the exe thread named `name` again, as CreateThread did (same start / param, a new TEB
/// and stack), reporting its parent-side thread id (the exe's thread registrations and allocators hold that id).
pub fn restart_exe_thread(name: &str) -> Option<u64> {
    let Some((old, proc_, ctx, _)) = EXE_THREADS.lock().unwrap().iter().find(|e| e.3 == name).cloned() else {
        hlog!("restart: no exe thread named {name}");
        return None;
    };
    if ctx == 0 {
        hlog!("restart: thread {name} has no engine start data");
        return None;
    }
    // a fresh copy each time (the entry writes its started flag +0x50 into it)
    let param = unsafe { crate::mem::heap_alloc(0x60, true) } as u64;
    unsafe { std::ptr::copy_nonoverlapping(ctx as *const u8, param as *mut u8, 0x60) };
    let start = proc_; // the UCRT procedure (the engine's thread entry), called with ctx as thread_start does
    let n = name.to_string();
    crate::mem::spawn_low(
        32 << 20,
        Box::new(move || {
            teb::setup_current_thread();
            FORCED_TID.with(|t| t.set(old));
            teb::run_tls_callbacks(2);
            let mut saved: u64 = 0;
            GUARD_RSP.with(|g| g.set(&mut saved as *mut u64 as u64));
            let code = unsafe { wh_call_guarded(start, param, &mut saved) } as u32;
            hlog!("restarted thread {n} ended with {code}");
        }),
    );
    hlog!("restart: exe thread {name} started again (start {start:#x}, param {param:#x}, reports tid {old})");
    Some(old)
}
unsafe extern "win64" fn CreateThread(_sa: A, stack: A, start: A, param: A, flags: A, tid_out: A) -> A {
    // the engine's thread start data (UCRT thread_start 0x20070C2D0, param = {procedure, ctx}; ctx: a boost::function
    // at +0 / +8.., names (char*) at +0x30 / +0x38, a started flag at +0x50 that the creator spins on; ctx lives on the
    // creator's stack): a private copy, so a fork()ed child can start the thread again
    let (ctx_copy, proc_) = unsafe {
        if start == 0x20070C2D0 && param != 0 && r64(param + 8) != 0 {
            hlog!("CreateThread: UCRT parameter {param:#x}: procedure {:#x}, context {:#x}", r64(param), r64(param + 8));
            let ctx = r64(param + 8);
            let c = crate::mem::heap_alloc(0x60, true) as u64;
            std::ptr::copy_nonoverlapping(ctx as *const u8, c as *mut u8, 0x60);
            for off in [0x30u64, 0x38] {
                let sp = r64(ctx + off);
                if sp != 0 {
                    let t = cstr(sp);
                    *((c + off) as *mut u64) = crate::strings::leak_cstr(&t);
                }
            }
            (c, r64(param))
        } else {
            (0, 0)
        }
    };
    let h = handles::insert(Obj::Thread { tid: 0, done: false, exit_code: 259, suspended: if flags & 4 != 0 { 1 } else { 0 } });
    let (tx, rx) = std::sync::mpsc::channel();
    let flags = flags & 0xFFFF_FFFF;
    let sz = (stack as usize).max(32 << 20);
    crate::mem::spawn_low(
        sz,
        Box::new(move || {
            teb::setup_current_thread();
            let tid = teb::gettid();
            handles::with(h, |o| {
                if let Obj::Thread { tid: t, .. } = o {
                    *t = tid;
                }
            });
            // registered before the creator returns and before the thread can name itself (otherwise the thread's own
            // naming exception can come first and its name is lost)
            EXE_THREADS.lock().unwrap().push((tid, proc_, ctx_copy, String::new()));
            tx.send(tid).unwrap();
            handles::wait_resumed(h);
            teb::run_tls_callbacks(2);
            let mut saved: u64 = 0;
            GUARD_RSP.with(|g| g.set(&mut saved as *mut u64 as u64));
            let code = unsafe { wh_call_guarded(start, param, &mut saved) } as u32;
            hlog!("thread {tid} ({h:#x}) ended with {code}");
            handles::with(h, |o| {
                if let Obj::Thread { done, exit_code, .. } = o {
                    *done = true;
                    *exit_code = code;
                }
            });
        }),
    );
    let tid = rx.recv().unwrap();
    hlog!("CreateThread(start {start:#x}, param {param:#x}, flags {flags:#x}) -> handle {h:#x} tid {tid}");
    if tid_out != 0 {
        unsafe { w32(tid_out, tid as u32) };
    }
    h
}
unsafe extern "win64" fn ExitThread(code: A) -> A {
    let p = GUARD_RSP.with(|g| g.get());
    if p == 0 {
        hlog!("ExitThread({code}) on a host-started thread; aborting");
        crate::run::abort_run(7);
    }
    let rsp = unsafe { *(p as *const u64) };
    unsafe { wh_longjmp_exit(rsp, code) }
}
unsafe extern "win64" fn FreeLibraryAndExitThread(_h: A, code: A) -> A {
    unsafe { ExitThread(code) }
}
unsafe extern "win64" fn ResumeThread(h: A) -> A {
    handles::with(h, |o| {
        if let Obj::Thread { suspended, .. } = o {
            let p = *suspended;
            if p > 0 {
                *suspended -= 1;
            }
            p as A
        } else {
            0xFFFF_FFFF
        }
    })
    .unwrap_or(0xFFFF_FFFF)
}
unsafe extern "win64" fn SetThreadPriority(_h: A, _p: A) -> A {
    1
}
unsafe extern "win64" fn SetThreadStackGuarantee(p: A) -> A {
    if p != 0 {
        unsafe { w32(p, 0x10000) };
    }
    1
}
// ------------------------------------------------------------------ fork gate (zygote)
//
// The engine's worker loop 0x2000FECC0 (arguments: the slot index and may_dispatch; slot = [0x200AB1A00] + index * 0x40)
// waits for a posted job like this (0x2000FED10..0x2000FED8F):
//
//     while [slot+0x38] == 0 {
//         if may_dispatch && byte [0x20F43C664] != 0 { (one-time init) ; dispatch 0x2000BCF60(0x200C70FE0, 0) }
//         Sleep(1)                                  ; the import 0x2008A6070, called at 0x2000FED84, returns to 0x2000FED8A
//     }
//
// At that Sleep the thread is between two iterations: the loop body takes no lock of its own and the dispatch, when
// it ran, has returned (its TryEnter/Enter/LeaveCriticalSection pairs close inside it). The loop's callers are the
// engine thread entries (0x200101B40 :hdd slot 6, 0x200101F30 :resources manager slot 7, 0x2000FD4F0 :sound slot 5,
// 0x2000FF590 slot 0, 0x2000FE670 slot 2, 0x2000FF3B0 slot 3, 0x2000FDD50 slot 4): each calls it first, after only
// thread set-up imports (GetCurrentThreadId; slot 3 CoInitializeEx, slot 4 GetCurrentThread + SetThreadPriority) and
// the store of its id at +0x30, and last, with nothing but the epilogue and `ret` after it (slot 3: a tail jump). So a
// thread inside this Sleep stand-in called from 0x2000FED8A runs no exe code and its frames hold no lock of their own
// (a lock left taken by a callee before the last call could never be released: the entry only returns after it). The
// fork gate parks such threads there, so a zygote's fork() never copies one of them in the middle of exe code (a lock
// it held would stay locked in the child). `HELD_CS == 0` (no Windows critical section taken) is checked at run time.
//
// Idle parking: while the zygote waits for hand-overs its world thread runs no exe code. A poller whose posted flag is
// 0 and that holds no critical section has no job to run. If its dispatch byte is 0 too, it can only read that flag
// and sleep again (the loop writes nothing), so it blocks in the gate, re-reading both every GATE_IDLE_RECHECK
// (`IdlePark::Stay`). In the 0.69d exe the dispatch byte [0x20F43C664] is set to 1 once (0x20009117B, on the
// `:resources manager` start) and never back to 0 (PROVEN), so the loop dispatches 0x2000BCF60 before every Sleep(1):
// about 935 times a second in an idle zygote. Such a poller parks for one GATE_IDLE_RECHECK per Sleep and then lets
// the loop run once (`IdlePark::Once`): the dispatch still runs, at most 25 ms late, about 40 times a second. A
// fork()ed child turns the gate off for good: its restarted threads poll exactly as before.

/// Return address of the worker loop's `call [Sleep]` at 0x2000FED84.
pub const WORKER_POLL_RET: u64 = 0x2000FED8A;
/// The worker loop's dispatch byte (read at 0x2000FED1C; its only writer, 0x20009117B, stores 1).
const GATE_PUMP_BYTE: u64 = 0x20F43C664;
/// The engine's worker slot table pointer (0x40 bytes per slot: +0 job, +0x30 thread id, +0x38 posted flag).
const GATE_SLOT_TABLE: u64 = 0x200AB1A00;
/// Slots searched for a poller's own (the range `run::fork_world` re-posts).
const GATE_SLOTS: u64 = 16;
/// A poller that polled within this many ms is in its loop (it polls about every 1.07 ms).
const GATE_ACTIVE_MS: u64 = 50;
/// A fork waits this long at most for the pollers in their loop to park; then it forks as it did before (logged).
const GATE_FORK_WAIT: Duration = Duration::from_millis(250);
/// An idle-parked poller re-reads its posted flag and the dispatch byte this often; with the dispatch byte set it parks
/// for this long per Sleep.
const GATE_IDLE_RECHECK: Duration = Duration::from_millis(25);

const GATE_OPEN: u8 = 0;
const GATE_IDLE: u8 = 1;
const GATE_FORK: u8 = 2;
/// A fork()ed child: the gate never acts again.
const GATE_OFF: u8 = 3;

static GATE_MODE: AtomicU8 = AtomicU8::new(GATE_OPEN);
static GATE: Mutex<()> = Mutex::new(());
static GATE_CV: Condvar = Condvar::new();
static GATE_EPOCH: OnceLock<Instant> = OnceLock::new();
/// Park / leave lines logged so far (the first 64 are logged).
static GATE_LOGS: AtomicU32 = AtomicU32::new(0);

/// One thread seen in the worker loop's Sleep.
struct Poller {
    tid: u64,
    /// its worker slot (its thread id at +0x30), or -1
    slot: i64,
    /// its latest poll, ms since GATE_EPOCH
    last_ms: AtomicU64,
    parked: AtomicBool,
    /// its first one-recheck idle park was logged (they come about 40 times a second: only the first is logged)
    once_logged: AtomicBool,
}
static POLLERS: Mutex<Vec<&'static Poller>> = Mutex::new(Vec::new());
thread_local! {
    static MY_POLLER: Cell<Option<&'static Poller>> = const { Cell::new(None) };
}

fn gate_ms() -> u64 {
    GATE_EPOCH.get_or_init(Instant::now).elapsed().as_millis() as u64
}

fn gate_log_ok() -> bool {
    GATE_LOGS.fetch_add(1, Ordering::Relaxed) < 64
}

fn gate_pump_byte() -> u8 {
    unsafe { std::ptr::read_volatile(GATE_PUMP_BYTE as *const u8) }
}

/// This thread's worker slot: the slot whose +0x30 holds the id the exe sees on this thread; -1 if none.
fn own_worker_slot() -> i64 {
    let me = GetCurrentThreadIdPub() as u32;
    unsafe {
        let t = r64(GATE_SLOT_TABLE);
        if t == 0 {
            return -1;
        }
        for i in 0..GATE_SLOTS {
            if r32(t + i * 0x40 + 0x30) == me {
                return i as i64;
            }
        }
    }
    -1
}

/// The posted flag of worker slot `slot` (None = no slot).
fn slot_posted(slot: i64) -> Option<u32> {
    if slot < 0 {
        return None;
    }
    unsafe {
        let t = r64(GATE_SLOT_TABLE);
        if t == 0 {
            return None;
        }
        Some((*((t + slot as u64 * 0x40 + 0x38) as *const AtomicU32)).load(Ordering::Acquire))
    }
}

/// How a worker poller may block in the zygote's idle gate.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IdlePark {
    /// It has a job (posted flag set), holds a critical section, or has no worker slot: it runs on.
    No,
    /// No job, no lock, but the dispatch byte is set: one GATE_IDLE_RECHECK, then its loop dispatches once.
    Once,
    /// No job, no lock, no dispatch: its loop can only read its posted flag, so it stays until that changes.
    Stay,
}

/// The idle gate's rule for a poller: `pump_byte` = the dispatch byte, `posted` = its slot's posted flag (None = no
/// slot found), `held_cs` = the critical sections it holds.
pub fn idle_park(pump_byte: u8, posted: Option<u32>, held_cs: i64) -> IdlePark {
    if posted != Some(0) || held_cs != 0 {
        IdlePark::No
    } else if pump_byte == 0 {
        IdlePark::Stay
    } else {
        IdlePark::Once
    }
}

/// The fork gate's census over the worker pollers, each (parked, latest poll in ms): (expected, parked, away). A
/// poller is expected to park when it is parked or polled within `active_ms` of `now_ms`; one that has not polled for
/// longer is away from its loop (it runs its job, or its thread ended) and cannot park.
pub fn poller_census(p: &[(bool, u64)], now_ms: u64, active_ms: u64) -> (usize, usize, usize) {
    let parked = p.iter().filter(|x| x.0).count();
    let active = p.iter().filter(|x| !x.0 && now_ms.saturating_sub(x.1) <= active_ms).count();
    (parked + active, parked, p.len() - parked - active)
}

/// The worker loop's Sleep(1) (called from 0x2000FED8A): note the poll, and park while the gate says so. true = the
/// thread was parked (its Sleep is over: it waited at least that long).
fn worker_poll_gate() -> bool {
    let me = match MY_POLLER.with(|c| c.get()) {
        Some(p) => p,
        None => {
            // under the gate's lock, which a fork holds across fork(): never copied in the middle of a log line
            let _g = GATE.lock().unwrap_or_else(|e| e.into_inner());
            let p: &'static Poller = Box::leak(Box::new(Poller {
                tid: teb::gettid(),
                slot: own_worker_slot(),
                last_ms: AtomicU64::new(0),
                parked: AtomicBool::new(false),
                once_logged: AtomicBool::new(false),
            }));
            POLLERS.lock().unwrap_or_else(|e| e.into_inner()).push(p);
            MY_POLLER.with(|c| c.set(Some(p)));
            hlog!("fork gate: thread {} polls in the worker loop's Sleep ({WORKER_POLL_RET:#x}), worker slot {}", p.tid, p.slot);
            p
        }
    };
    me.last_ms.store(gate_ms(), Ordering::Relaxed);
    let held = HELD_CS.with(|h| h.get());
    let mode = GATE_MODE.load(Ordering::Acquire);
    if mode == GATE_OPEN || mode == GATE_OFF || held != 0 {
        return false;
    }
    let first = if mode == GATE_IDLE { idle_park(gate_pump_byte(), slot_posted(me.slot), held) } else { IdlePark::Stay };
    if first == IdlePark::No {
        return false;
    }
    // a one-recheck park is logged once per poller (they repeat about 40 times a second); every other park is counted
    // against the gate's 64 lines
    let quiet = first == IdlePark::Once && me.once_logged.swap(true, Ordering::Relaxed);
    let mut g = GATE.lock().unwrap_or_else(|e| e.into_inner());
    // the lines are written while the gate's lock is held: a fork (which holds it across fork()) never copies a
    // poller in the middle of a log line
    me.parked.store(true, Ordering::SeqCst);
    GATE_CV.notify_all();
    if !quiet && gate_log_ok() {
        let how = match (mode, first) {
            (GATE_FORK, _) => "fork",
            (_, IdlePark::Once) => "idle, one recheck per Sleep while the dispatch byte is set",
            _ => "idle",
        };
        hlog!("fork gate: thread {} (worker slot {}) parked in Sleep ({how})", me.tid, me.slot);
    }
    // a one-recheck park ends at its deadline, not at the first wake (other pollers' park / leave notifications)
    let until = Instant::now() + GATE_IDLE_RECHECK;
    loop {
        let m = GATE_MODE.load(Ordering::Acquire);
        if m == GATE_OPEN || m == GATE_OFF {
            break;
        }
        let mut wait = GATE_IDLE_RECHECK;
        if m == GATE_IDLE {
            match idle_park(gate_pump_byte(), slot_posted(me.slot), held) {
                IdlePark::No => break,
                IdlePark::Once => {
                    let now = Instant::now();
                    if now >= until {
                        break;
                    }
                    wait = until - now;
                }
                IdlePark::Stay => {}
            }
        }
        g = GATE_CV.wait_timeout(g, wait).unwrap_or_else(|e| e.into_inner()).0;
    }
    me.parked.store(false, Ordering::SeqCst);
    // back in its loop now: the fork census counts it as polling from here, not from when it entered the Sleep
    me.last_ms.store(gate_ms(), Ordering::Relaxed);
    GATE_CV.notify_all();
    if !quiet && gate_log_ok() {
        hlog!(
            "fork gate: thread {} (worker slot {}) leaves the gate (posted flag {:?}, dispatch byte {})",
            me.tid,
            me.slot,
            slot_posted(me.slot),
            gate_pump_byte()
        );
    }
    drop(g);
    true
}

/// The zygote between hand-overs (its world thread runs no exe code): `on` = idle worker pollers may block in the
/// gate (`idle_park`); off = they poll again.
pub fn fork_gate_idle(on: bool) {
    if GATE_MODE.load(Ordering::SeqCst) == GATE_OFF {
        return;
    }
    let _g = GATE.lock().unwrap_or_else(|e| e.into_inner());
    GATE_MODE.store(if on { GATE_IDLE } else { GATE_OPEN }, Ordering::SeqCst);
    GATE_CV.notify_all();
    let n = POLLERS.lock().unwrap_or_else(|e| e.into_inner()).len();
    hlog!("fork gate: idle parking {}; dispatch byte [{GATE_PUMP_BYTE:#x}] = {}; {n} worker poller(s) known", if on { "on" } else { "off" }, gate_pump_byte());
}

/// Held across fork(): the gate's lock, and the mode the parent returns to.
pub struct ForkGate {
    guard: Option<std::sync::MutexGuard<'static, ()>>,
    prev: u8,
    /// every poller still in its loop parked within GATE_FORK_WAIT (false = TIMEOUT)
    pub complete: bool,
}

/// Before fork(): every worker poller that reaches its Sleep parks (mode fork); wait until each poller still in its
/// loop is parked (at most GATE_FORK_WAIT, then fork as before), and keep the gate's lock across the fork.
pub fn fork_gate_close() -> ForkGate {
    let prev = GATE_MODE.swap(GATE_FORK, Ordering::SeqCst);
    if prev == GATE_OFF {
        GATE_MODE.store(GATE_OFF, Ordering::SeqCst);
        return ForkGate { guard: None, prev, complete: true };
    }
    let t0 = Instant::now();
    let mut g = GATE.lock().unwrap_or_else(|e| e.into_inner());
    let complete = loop {
        let (snap, tids): (Vec<(bool, u64)>, Vec<String>) = {
            let ps = POLLERS.lock().unwrap_or_else(|e| e.into_inner());
            (ps.iter().map(|p| (p.parked.load(Ordering::SeqCst), p.last_ms.load(Ordering::Relaxed))).collect(), ps.iter().filter(|p| p.parked.load(Ordering::SeqCst)).map(|p| p.tid.to_string()).collect())
        };
        let (expected, parked, away) = poller_census(&snap, gate_ms(), GATE_ACTIVE_MS);
        let done = parked >= expected;
        if done || t0.elapsed() >= GATE_FORK_WAIT {
            hlog!(
                "fork gate: {parked} of {expected} worker poller(s) parked in Sleep ({WORKER_POLL_RET:#x}) after {} us{}{}; {away} away from their loop",
                t0.elapsed().as_micros(),
                if tids.is_empty() { String::new() } else { format!(" (threads {})", tids.join(", ")) },
                if done { "" } else { ": TIMEOUT, not every poller parked" }
            );
            break done;
        }
        g = GATE_CV.wait_timeout(g, Duration::from_millis(2)).unwrap_or_else(|e| e.into_inner()).0;
    };
    ForkGate { guard: Some(g), prev, complete }
}

impl ForkGate {
    /// The parent after fork(): back to the mode before (an idle zygote keeps its pollers parked; an open gate lets
    /// them go).
    pub fn reopen(self) {
        let ForkGate { guard, prev, .. } = self;
        GATE_MODE.store(prev, Ordering::SeqCst);
        GATE_CV.notify_all();
        drop(guard);
    }

    /// The child after fork(): the gate is off for good. None of the parked threads exists here; the threads the
    /// child starts again poll exactly as before. The lock stays taken (nothing in the child takes it again).
    pub fn child(self) {
        GATE_MODE.store(GATE_OFF, Ordering::SeqCst);
        std::mem::forget(self);
    }

    /// A zygote forked from the base process, which forks match children itself: the gate stays usable. Open mode,
    /// the poller records of the base's threads (none of which exists here; they would count as parked in every later
    /// census) dropped, the log budget reset, and the gate's lock released (this thread holds it: it took it before the
    /// fork). Every POLLERS access happens under the gate's lock, so that list is not locked by a missing thread.
    /// Returns the number of stale poller records dropped.
    pub fn child_keep(self) -> usize {
        let ForkGate { guard, .. } = self;
        let stale = {
            let mut p = POLLERS.lock().unwrap_or_else(|e| e.into_inner());
            let n = p.len();
            p.clear();
            n
        };
        GATE_LOGS.store(0, Ordering::Relaxed);
        GATE_MODE.store(GATE_OPEN, Ordering::SeqCst);
        drop(guard);
        stale
    }
}

unsafe extern "win64" fn Sleep(ms: A) -> A {
    // the render pool's full callback (a producer waiting for records): retire the queued commands
    if ms == 0 && crate::thunks::caller() == crate::world::POOL_FULL_SLEEP_RET {
        crate::world::retire_at_capacity();
    }
    // the fork gate: only the engine worker loop's Sleep(1), never in a fork()ed child
    if ms != 0 && GATE_MODE.load(Ordering::Relaxed) != GATE_OFF && crate::thunks::caller() == WORKER_POLL_RET && worker_poll_gate() {
        return 0;
    }
    if ms == 0 {
        std::thread::yield_now();
    } else {
        std::thread::sleep(Duration::from_millis(ms as u32 as u64));
    }
    0
}
unsafe extern "win64" fn SleepEx(ms: A, _alertable: A) -> A {
    unsafe { Sleep(ms) };
    0
}
unsafe extern "win64" fn SwitchToThread() -> A {
    // the render pool's full callback (a producer waiting for records): retire the queued commands
    if crate::thunks::caller() == crate::world::POOL_FULL_SWITCH_RET {
        crate::world::retire_at_capacity();
    }
    std::thread::yield_now();
    1
}
unsafe extern "win64" fn CreateEventA(_sa: A, manual: A, initial: A, _name: A) -> A {
    handles::insert(Obj::Event { manual: manual as u32 != 0, signaled: initial as u32 != 0 })
}
unsafe extern "win64" fn CreateEventW(sa: A, manual: A, initial: A, name: A) -> A {
    unsafe { CreateEventA(sa, manual, initial, name) }
}
unsafe extern "win64" fn SetEvent(h: A) -> A {
    handles::with(h, |o| {
        if let Obj::Event { signaled, .. } = o {
            *signaled = true;
        }
    })
    .is_some() as A
}
unsafe extern "win64" fn ResetEvent(h: A) -> A {
    handles::with(h, |o| {
        if let Obj::Event { signaled, .. } = o {
            *signaled = false;
        }
    })
    .is_some() as A
}
unsafe extern "win64" fn CreateSemaphoreA(_sa: A, init: A, max: A, _n: A) -> A {
    handles::insert(Obj::Semaphore { count: init as i32 as i64, max: max as i32 as i64 })
}
unsafe extern "win64" fn ReleaseSemaphore(h: A, n: A, prev: A) -> A {
    handles::with(h, |o| {
        if let Obj::Semaphore { count, max } = o {
            if prev != 0 {
                unsafe { w32(prev, *count as u32) };
            }
            *count = (*count + n as i32 as i64).min(*max);
        }
    })
    .is_some() as A
}
unsafe extern "win64" fn CreateMutexW(_sa: A, initial: A, name: A) -> A {
    let nm = if name != 0 { unsafe { wstr(name) } } else { String::new() };
    let owner = if initial as u32 != 0 { teb::gettid() } else { 0 };
    let h = handles::insert(Obj::Mutex { owner, count: (owner != 0) as u32 });
    hlog!("CreateMutexW({nm}) -> {h:#x}");
    err(0);
    h
}
unsafe extern "win64" fn CreateWaitableTimerA(_sa: A, manual: A, _n: A) -> A {
    handles::insert(Obj::Timer { manual: manual as u32 != 0, signaled: false, due: None, period: Duration::ZERO })
}
unsafe extern "win64" fn SetWaitableTimer(h: A, due: A, period: A, cb: A, _arg: A, _resume: A) -> A {
    if cb != 0 {
        hlog!("SetWaitableTimer with a completion routine ({cb:#x}) is not supported; aborting");
        crate::run::abort_run(8);
    }
    let d = unsafe { *(due as *const i64) };
    let when = if d < 0 {
        Instant::now() + Duration::from_nanos((-d) as u64 * 100)
    } else {
        let now = now_filetime() as i64;
        Instant::now() + Duration::from_nanos(((d - now).max(0) as u64) * 100)
    };
    handles::with(h, |o| {
        if let Obj::Timer { signaled, due, period: p, .. } = o {
            *signaled = false;
            *due = Some(when);
            *p = Duration::from_millis(period as u32 as u64);
        }
    })
    .is_some() as A
}
unsafe extern "win64" fn WaitForSingleObject(h: A, ms: A) -> A {
    handles::wait(&[h], false, ms as u32) as A
}
unsafe extern "win64" fn WaitForSingleObjectEx(h: A, ms: A, _al: A) -> A {
    handles::wait(&[h], false, ms as u32) as A
}
/// Return address of the exe timer loop's idle wait (WaitForMultipleObjects called at 0x2000AEC3C in 0x2000AE860, the
/// loop of the `:hdd` / `:resources manager` threads): a thread in it runs no exe code and its loop holds no lock.
pub const TIMER_IDLE_WAIT_RET: u64 = 0x2000AEC42;
unsafe extern "win64" fn WaitForMultipleObjects(n: A, arr: A, all: A, ms: A) -> A {
    let hs = unsafe { std::slice::from_raw_parts(arr as *const u64, n as u32 as usize) }.to_vec();
    handles::wait_marked(&hs, all as u32 != 0, ms as u32, crate::thunks::caller() == TIMER_IDLE_WAIT_RET) as A
}
unsafe extern "win64" fn CloseHandle(h: A) -> A {
    if h == handles::PSEUDO_PROCESS || h == handles::PSEUDO_THREAD {
        return 1;
    }
    let ok = handles::close(h);
    if !ok {
        hlog!("CloseHandle({h:#x}): unknown handle");
    }
    1
}
unsafe extern "win64" fn DuplicateHandle(_sp: A, src: A, _tp: A, out: A, _acc: A, _inh: A, _opt: A) -> A {
    let r = if src == handles::PSEUDO_THREAD {
        handles::insert(Obj::Thread { tid: teb::gettid(), done: false, exit_code: 259, suspended: 0 })
    } else if src == handles::PSEUDO_PROCESS {
        handles::insert(Obj::Process)
    } else {
        hlog!("DuplicateHandle({src:#x}) aliases the same handle");
        src
    };
    if out != 0 {
        unsafe { w64(out, r) };
    }
    1
}
unsafe extern "win64" fn CreateIoCompletionPort(file: A, existing: A, key: A, _n: A) -> A {
    let port = if existing != 0 { existing } else { handles::insert(Obj::Iocp { queue: Default::default() }) };
    if file != INVALID_HANDLE {
        handles::iocp_associate(file, port, key);
    }
    port
}
unsafe extern "win64" fn GetQueuedCompletionStatus(port: A, bytes: A, key: A, ov: A, ms: A) -> A {
    match handles::iocp_get(port, ms as u32) {
        Some((b, k, o)) => {
            unsafe {
                w32(bytes, b);
                w64(key, k);
                w64(ov, o);
            }
            1
        }
        None => {
            unsafe { w64(ov, 0) };
            err(258);
            0
        }
    }
}
unsafe extern "win64" fn PostQueuedCompletionStatus(port: A, bytes: A, key: A, ov: A) -> A {
    handles::iocp_post(port, bytes as u32, key, ov) as A
}
unsafe extern "win64" fn GetOverlappedResult(_f: A, ov: A, bytes: A, _w: A) -> A {
    unsafe { w32(bytes, r64(ov + 8) as u32) };
    1
}

// ------------------------------------------------------------------ files

fn log_files() -> bool {
    crate::run::verbose_files()
}
fn create_file(name: String, access: A, disp: A, flags: A) -> A {
    let (access, disp, flags) = (access & 0xFFFF_FFFF, disp & 0xFFFF_FFFF, flags & 0xFFFF_FFFF);
    match files::open(&name, access as u32, disp as u32, flags as u32) {
        Ok(f) => {
            let p = f.path.display().to_string();
            let h = handles::insert(Obj::File(std::sync::Arc::new(Mutex::new(f))));
            if log_files() {
                hlog!("CreateFile({name}, acc {access:#x}, disp {disp}) -> {h:#x} [{p}]");
            }
            files::note_open(name);
            err(0);
            h
        }
        Err(e) => {
            if log_files() {
                hlog!("CreateFile({name}, acc {access:#x}, disp {disp}) -> error {e}");
            }
            err(e);
            INVALID_HANDLE
        }
    }
}
unsafe extern "win64" fn CreateFileW(name: A, access: A, _share: A, _sa: A, disp: A, flags: A, _t: A) -> A {
    create_file(unsafe { wstr(name) }, access, disp, flags)
}
unsafe extern "win64" fn CreateFileA(name: A, access: A, _share: A, _sa: A, disp: A, flags: A, _t: A) -> A {
    create_file(unsafe { cstr(name) }, access, disp, flags)
}
fn complete_overlapped(h: A, ov: A, n: u32) {
    unsafe {
        w64(ov, 0);
        w64(ov + 8, n as u64);
        let ev = r64(ov + 0x18);
        if ev != 0 {
            SetEvent(ev & !1);
        }
    }
    if let Some((port, key)) = handles::iocp_for_file(h) {
        handles::iocp_post(port, n, key, ov);
    }
}
unsafe extern "win64" fn ReadFile(h: A, buf: A, n: A, read: A, ov: A) -> A {
    use std::os::unix::fs::FileExt;
    let Some(f) = handles::file(h) else {
        err(6);
        return 0;
    };
    let mut f = f.lock().unwrap();
    let off = if ov != 0 { unsafe { r32(ov + 0x10) as u64 | ((r32(ov + 0x14) as u64) << 32) } } else { f.pos };
    let dst = unsafe { std::slice::from_raw_parts_mut(buf as *mut u8, n as u32 as usize) };
    let files::FileKind::File(file) = &f.kind else {
        err(5);
        return 0;
    };
    let mut got = 0usize;
    while got < dst.len() {
        // reads go in 8 MB pieces: one large read from a 9p-mounted game drive fails with ENOMEM
        let end = dst.len().min(got + (8 << 20));
        match file.read_at(&mut dst[got..end], off + got as u64) {
            Ok(0) => break,
            Ok(k) => got += k,
            Err(e) => {
                hlog!("ReadFile({}) error {e} (buffer {buf:#x}, {n:#x} bytes, offset {:#x}, done {got:#x})", f.win_path, off);
                err(30);
                return 0;
            }
        }
    }
    if ov == 0 {
        f.pos = off + got as u64;
    }
    if read != 0 {
        unsafe { w32(read, got as u32) };
    }
    if ov != 0 {
        drop(f);
        if got == 0 && n != 0 {
            unsafe { w64(ov, 0xC000_0011) }; // STATUS_END_OF_FILE
            err(38);
            return 0;
        }
        complete_overlapped(h, ov, got as u32);
    }
    1
}
unsafe extern "win64" fn WriteFile(h: A, buf: A, n: A, written: A, ov: A) -> A {
    use std::os::unix::fs::FileExt;
    let Some(f) = handles::file(h) else {
        err(6);
        return 0;
    };
    let mut f = f.lock().unwrap();
    let off = if ov != 0 { unsafe { r32(ov + 0x10) as u64 | ((r32(ov + 0x14) as u64) << 32) } } else { f.pos };
    let src = unsafe { std::slice::from_raw_parts(buf as *const u8, n as u32 as usize) };
    let files::FileKind::File(file) = &f.kind else {
        err(5);
        return 0;
    };
    if let Err(e) = file.write_all_at(src, off) {
        hlog!("WriteFile({}) error {e}", f.win_path);
        err(5);
        return 0;
    }
    if ov == 0 {
        f.pos = off + src.len() as u64;
    }
    if written != 0 {
        unsafe { w32(written, src.len() as u32) };
    }
    if ov != 0 {
        drop(f);
        complete_overlapped(h, ov, src.len() as u32);
    }
    1
}
fn file_len(f: &files::FileObj) -> u64 {
    match &f.kind {
        files::FileKind::File(x) => x.metadata().map(|m| m.len()).unwrap_or(0),
        _ => 0,
    }
}
unsafe extern "win64" fn SetFilePointerEx(h: A, dist: A, newpos: A, method: A) -> A {
    let Some(f) = handles::file(h) else {
        err(6);
        return 0;
    };
    let mut f = f.lock().unwrap();
    let base = match method {
        0 => 0i64,
        1 => f.pos as i64,
        _ => file_len(&f) as i64,
    };
    let p = base + dist as i64;
    if p < 0 {
        err(131);
        return 0;
    }
    f.pos = p as u64;
    if newpos != 0 {
        unsafe { w64(newpos, p as u64) };
    }
    1
}
unsafe extern "win64" fn SetFilePointer(h: A, lo: A, hi: A, method: A) -> A {
    let d = if hi != 0 { ((unsafe { r32(hi) } as u64) << 32) | (lo as u32 as u64) } else { lo as i32 as i64 as u64 };
    let mut np = 0u64;
    if unsafe { SetFilePointerEx(h, d, &mut np as *mut u64 as A, method) } == 0 {
        return 0xFFFF_FFFF;
    }
    if hi != 0 {
        unsafe { w32(hi, (np >> 32) as u32) };
    }
    np & 0xFFFF_FFFF
}
unsafe extern "win64" fn GetFileSizeEx(h: A, out: A) -> A {
    let Some(f) = handles::file(h) else {
        err(6);
        return 0;
    };
    let n = file_len(&f.lock().unwrap());
    unsafe { w64(out, n) };
    1
}
unsafe extern "win64" fn GetFileType(h: A) -> A {
    if handles::file(h).is_some() { 1 } else { 0 }
}
unsafe extern "win64" fn GetFileInformationByHandle(h: A, p: A) -> A {
    use std::os::unix::fs::MetadataExt;
    let Some(f) = handles::file(h) else {
        err(6);
        return 0;
    };
    let f = f.lock().unwrap();
    let Ok(md) = std::fs::metadata(&f.path) else {
        err(2);
        return 0;
    };
    let (c, a, m) = files::times_of(&md);
    unsafe {
        zero(p, 52);
        w32(p, files::attrs_of(&md));
        w64(p + 4, c);
        w64(p + 12, a);
        w64(p + 20, m);
        w32(p + 28, 0x1234_5678);
        w32(p + 32, (md.len() >> 32) as u32);
        w32(p + 36, md.len() as u32);
        w32(p + 40, 1);
        w32(p + 44, (md.ino() >> 32) as u32);
        w32(p + 48, md.ino() as u32);
    }
    1
}
unsafe extern "win64" fn FlushFileBuffers(_h: A) -> A {
    1
}
unsafe extern "win64" fn SetEndOfFile(h: A) -> A {
    let Some(f) = handles::file(h) else {
        err(6);
        return 0;
    };
    let f = f.lock().unwrap();
    if let files::FileKind::File(x) = &f.kind {
        let _ = x.set_len(f.pos);
    }
    1
}
fn attrs_of_path(name: &str) -> Option<std::fs::Metadata> {
    std::fs::metadata(files::read_path(name)).ok()
}
unsafe extern "win64" fn GetFileAttributesW(name: A) -> A {
    let n = unsafe { wstr(name) };
    match attrs_of_path(&n) {
        Some(md) => files::attrs_of(&md) as A,
        None => {
            err(2);
            0xFFFF_FFFF
        }
    }
}
unsafe extern "win64" fn GetFileAttributesA(name: A) -> A {
    let n = unsafe { cstr(name) };
    match attrs_of_path(&n) {
        Some(md) => files::attrs_of(&md) as A,
        None => {
            err(2);
            0xFFFF_FFFF
        }
    }
}
unsafe extern "win64" fn GetFileAttributesExW(name: A, _lvl: A, p: A) -> A {
    let n = unsafe { wstr(name) };
    match attrs_of_path(&n) {
        Some(md) => {
            let (c, a, m) = files::times_of(&md);
            unsafe {
                w32(p, files::attrs_of(&md));
                w64(p + 4, c);
                w64(p + 12, a);
                w64(p + 20, m);
                w32(p + 28, (md.len() >> 32) as u32);
                w32(p + 32, md.len() as u32);
            }
            1
        }
        None => {
            if log_files() {
                hlog!("GetFileAttributesExW({n}) -> not found");
            }
            err(2);
            0
        }
    }
}
unsafe fn fill_find(p: A, name: &str, md: &std::fs::Metadata) {
    let (c, a, m) = files::times_of(md);
    unsafe {
        zero(p, 592);
        w32(p, files::attrs_of(md));
        w64(p + 4, c);
        w64(p + 12, a);
        w64(p + 20, m);
        w32(p + 28, (md.len() >> 32) as u32);
        w32(p + 32, md.len() as u32);
        put_wstr(p + 44, 260, name);
    }
}
unsafe extern "win64" fn FindFirstFileExW(name: A, _lvl: A, data: A, _op: A, _f: A, _fl: A) -> A {
    let n = unsafe { wstr(name) };
    match files::find(&n) {
        Some(mut fo) => {
            let (nm, md) = &fo.entries[0];
            unsafe { fill_find(data, nm, md) };
            fo.next = 1;
            if log_files() {
                hlog!("FindFirstFileExW({n}) -> {} entries", fo.entries.len());
            }
            handles::insert(Obj::Find(std::sync::Arc::new(Mutex::new(fo))))
        }
        None => {
            if log_files() {
                hlog!("FindFirstFileExW({n}) -> none");
            }
            err(2);
            INVALID_HANDLE
        }
    }
}
unsafe extern "win64" fn FindNextFileW(h: A, data: A) -> A {
    let Some(f) = handles::find(h) else {
        err(6);
        return 0;
    };
    let mut f = f.lock().unwrap();
    if f.next >= f.entries.len() {
        err(18);
        return 0;
    }
    let (nm, md) = &f.entries[f.next];
    unsafe { fill_find(data, nm, md) };
    f.next += 1;
    1
}
unsafe extern "win64" fn FindClose(h: A) -> A {
    handles::close(h);
    1
}
unsafe extern "win64" fn CreateDirectoryW(name: A, _sa: A) -> A {
    let n = unsafe { wstr(name) };
    if files::read_path(&n).is_dir() {
        err(183);
        return 0;
    }
    let p = files::write_path(&n);
    let ok = std::fs::create_dir_all(&p).is_ok();
    hlog!("CreateDirectoryW({n}) -> {} [{}]", ok, p.display());
    ok as A
}
unsafe extern "win64" fn DeleteFileW(name: A) -> A {
    let n = unsafe { wstr(name) };
    let (_, ov) = files::host_paths(&n);
    let p = ov.unwrap_or_else(|| files::host_paths(&n).0);
    hlog!("DeleteFileW({n}) -> overlay/private only");
    std::fs::remove_file(p).is_ok() as A
}
unsafe extern "win64" fn GetFullPathNameW(name: A, n: A, buf: A, part: A) -> A {
    let s = files::win_abs(&unsafe { wstr(name) });
    let (k, ok) = unsafe { put_wstr(buf, n as usize, &s) };
    if ok && part != 0 {
        let idx = s.rfind('\\').map(|i| i + 1).unwrap_or(0);
        let off = s[..idx].encode_utf16().count();
        unsafe { w64(part, buf + 2 * off as u64) };
    }
    k as A
}
unsafe extern "win64" fn GetDiskFreeSpaceExW(_d: A, a: A, t: A, f: A) -> A {
    unsafe {
        if a != 0 {
            w64(a, 100 << 30);
        }
        if t != 0 {
            w64(t, 1000 << 30);
        }
        if f != 0 {
            w64(f, 100 << 30);
        }
    }
    1
}
unsafe extern "win64" fn GetDriveTypeW(_d: A) -> A {
    3
}

// ------------------------------------------------------------------ table

macro_rules! t {
    ($($n:ident),* $(,)?) => { vec![$((stringify!($n), $n as *const () as usize)),*] };
}

pub fn table() -> Vec<(&'static str, usize)> {
    t![
        GetCommandLineW, GetCommandLineA, GetModuleHandleW, GetModuleHandleA, GetModuleHandleExW, GetModuleFileNameW,
        GetModuleFileNameA, LoadLibraryA, LoadLibraryW, LoadLibraryExW, FreeLibrary, GetProcAddress, GetCurrentProcess,
        GetCurrentProcessId, GetCurrentThread, GetCurrentThreadId, ExitProcess, TerminateProcess, GetStartupInfoW,
        GetEnvironmentStringsW, FreeEnvironmentStringsW, GetEnvironmentVariableA, GetEnvironmentVariableW,
        SetEnvironmentVariableW, GetCurrentDirectoryW, GetSystemDirectoryW, GetVersion, VerifyVersionInfoW,
        VerSetConditionMask, IsProcessorFeaturePresent, GetSystemInfo, SetPriorityClass, VirtualAlloc, VirtualFree,
        VirtualQuery, GetProcessHeap, HeapAlloc, HeapReAlloc, HeapFree, HeapSize, HeapWalk, HeapValidate, GlobalAlloc,
        GlobalFree, GlobalLock, GlobalUnlock, LocalAlloc, LocalFree, GlobalMemoryStatusEx, GlobalMemoryStatus, TlsAlloc,
        TlsFree, TlsGetValue, TlsSetValue, GetLastError, SetLastError, OutputDebugStringA, IsDebuggerPresent, RtlCaptureContext,
        SetUnhandledExceptionFilter, UnhandledExceptionFilter, RaiseException, EncodePointer, DecodePointer,
        FormatMessageA, GetConsoleWindow, AttachConsole, AllocConsole, GetStdHandle, SetStdHandle, SetConsoleCtrlHandler,
        GetConsoleCP, GetConsoleMode, MultiByteToWideChar, WideCharToMultiByte, LCMapStringW, LCMapStringA,
        GetStringTypeW, GetStringTypeExA, CompareStringW, FoldStringW, GetCPInfo, GetACP, GetOEMCP, IsValidCodePage,
        IsValidLocale, GetUserDefaultLCID, GetLocaleInfoW, QueryPerformanceFrequency, QueryPerformanceCounter,
        GetTickCount, GetSystemTimeAsFileTime, GetLocalTime, FileTimeToSystemTime, SystemTimeToTzSpecificLocalTime,
        GetTimeZoneInformation, InitializeCriticalSection, InitializeCriticalSectionAndSpinCount,
        InitializeCriticalSectionEx, DeleteCriticalSection, EnterCriticalSection, TryEnterCriticalSection,
        LeaveCriticalSection, InitializeSListHead, InterlockedPushEntrySList, CreateThread, ExitThread,
        FreeLibraryAndExitThread, ResumeThread, SetThreadPriority, SetThreadStackGuarantee, Sleep, SleepEx,
        SwitchToThread, CreateEventA, CreateEventW, SetEvent, ResetEvent, CreateSemaphoreA, ReleaseSemaphore,
        CreateMutexW, CreateWaitableTimerA, SetWaitableTimer, WaitForSingleObject, WaitForSingleObjectEx,
        WaitForMultipleObjects, CloseHandle, DuplicateHandle, CreateIoCompletionPort, GetQueuedCompletionStatus,
        PostQueuedCompletionStatus, GetOverlappedResult, CreateFileW, CreateFileA, ReadFile, WriteFile,
        SetFilePointerEx, SetFilePointer, GetFileSizeEx, GetFileType, GetFileInformationByHandle, FlushFileBuffers,
        SetEndOfFile, GetFileAttributesW, GetFileAttributesA, GetFileAttributesExW, FindFirstFileExW, FindNextFileW,
        FindClose, CreateDirectoryW, DeleteFileW, GetFullPathNameW, GetDiskFreeSpaceExW, GetDriveTypeW,
    ]
}

#[cfg(test)]
mod gate_tests {
    use super::*;

    #[test]
    fn an_idle_poller_parks_only_while_it_has_no_job_and_no_lock() {
        assert_eq!(idle_park(0, Some(0), 0), IdlePark::Stay, "no dispatch, no job, no lock: the loop only reads its flag");
        assert_eq!(idle_park(1, Some(0), 0), IdlePark::Once, "the dispatch byte is set: one recheck, then it dispatches");
        assert_eq!(idle_park(0, Some(1), 0), IdlePark::No, "a job is posted: it must run it");
        assert_eq!(idle_park(1, Some(1), 0), IdlePark::No, "a job is posted, dispatch byte set");
        assert_eq!(idle_park(0, None, 0), IdlePark::No, "no worker slot found: not parked while idle");
        assert_eq!(idle_park(1, Some(0), 1), IdlePark::No, "it holds a critical section");
    }

    #[test]
    fn the_fork_waits_for_every_poller_still_in_its_loop() {
        // (parked, last poll ms); now 1000, active within 50 ms
        assert_eq!(poller_census(&[], 1000, 50), (0, 0, 0), "no poller: fork at once");
        assert_eq!(poller_census(&[(true, 10)], 1000, 50), (1, 1, 0), "parked long ago still counts as parked");
        assert_eq!(poller_census(&[(false, 999)], 1000, 50), (1, 0, 0), "polling: the fork waits for it");
        assert_eq!(poller_census(&[(false, 900)], 1000, 50), (0, 0, 1), "away from its loop (a job, or ended): not waited for");
        assert_eq!(poller_census(&[(true, 0), (false, 990), (false, 100)], 1000, 50), (2, 1, 1));
        assert_eq!(poller_census(&[(false, 1005)], 1000, 50), (1, 0, 0), "a poll stamped after `now`");
    }

    #[test]
    fn the_clock_offset_takes_whole_seconds_only() {
        assert_eq!(clock_offset(None), Duration::ZERO);
        assert_eq!(clock_offset(Some("3600")), Duration::from_secs(3600));
        assert_eq!(clock_offset(Some(" 20000 ")), Duration::from_secs(20000));
        assert_eq!(clock_offset(Some("1.5")), Duration::ZERO);
        assert_eq!(clock_offset(Some("-5")), Duration::ZERO);
        assert_eq!(clock_offset(Some("")), Duration::ZERO);
    }
}
