//! Fake TEB / PEB / TLS per thread, installed as the GS base (glibc uses FS, GS is free).
//!
//! Fields the exe reads (all PROVEN):
//!   gs:[0x08] StackBase, gs:[0x10] StackLimit (__chkstk 0x2006DC1A0), gs:[0x30] TEB self,
//!   gs:[0x58] ThreadLocalStoragePointer (428 sites read slot 0 directly), gs:[0x60] PEB
//!   (PEB+0xBC NtGlobalFlag, PEB+0x20 ProcessParameters).
//! Dynamic TLS (TlsAlloc/TlsGetValue) lives in TlsSlots (TEB+0x1480, 64) and a host expansion array.

use std::cell::Cell;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

pub const TEB_SIZE: usize = 0x2000;
pub const TEB_STACK_BASE: usize = 0x08;
pub const TEB_STACK_LIMIT: usize = 0x10;
pub const TEB_SELF: usize = 0x30;
pub const TEB_PID: usize = 0x40;
pub const TEB_TID: usize = 0x48;
pub const TEB_TLS_PTR: usize = 0x58;
pub const TEB_PEB: usize = 0x60;
pub const TEB_LAST_ERROR: usize = 0x68;
pub const TEB_TLS_SLOTS: usize = 0x1480;
pub const TEB_TLS_EXPANSION: usize = 0x1780;
pub const TLS_SLOTS: usize = 64;
pub const TLS_EXPANSION: usize = 1024;

/// Static TLS template (from the TLS directory), set by the loader.
pub struct TlsTemplate {
    pub start: usize,
    pub len: usize,
    pub zero_fill: usize,
    pub callbacks: Vec<usize>,
}
pub static TLS_TEMPLATE: OnceLock<TlsTemplate> = OnceLock::new();
pub static IMAGE_BASE: AtomicU64 = AtomicU64::new(0);
pub const FAKE_PROCESS_HEAP: u64 = 0x7FF0_0000_2000;

static PEB: OnceLock<usize> = OnceLock::new();

thread_local! {
    static TEB: Cell<usize> = const { Cell::new(0) };
}

fn zalloc(n: usize) -> usize {
    let p = crate::mem::low_mmap(n, libc::PROT_READ | libc::PROT_WRITE);
    assert!(p != 0);
    p
}

pub fn peb() -> usize {
    *PEB.get_or_init(|| {
        let p = zalloc(0x2000);
        let params = p + 0x1000; // zeroed RTL_USER_PROCESS_PARAMETERS
        unsafe {
            *((p + 0x10) as *mut u64) = IMAGE_BASE.load(Ordering::Relaxed);
            *((p + 0x20) as *mut u64) = params as u64;
            *((p + 0x30) as *mut u64) = FAKE_PROCESS_HEAP;
        }
        p
    })
}

pub fn gettid() -> u64 {
    unsafe { libc::syscall(libc::SYS_gettid) as u64 }
}

/// Bounds of the current pthread's stack (low, high).
pub fn current_stack() -> (usize, usize) {
    unsafe {
        let mut attr: libc::pthread_attr_t = std::mem::zeroed();
        assert_eq!(libc::pthread_getattr_np(libc::pthread_self(), &mut attr), 0);
        let mut addr: *mut libc::c_void = std::ptr::null_mut();
        let mut size: libc::size_t = 0;
        libc::pthread_attr_getstack(&attr, &mut addr, &mut size);
        let mut guard: libc::size_t = 0;
        libc::pthread_attr_getguardsize(&attr, &mut guard);
        libc::pthread_attr_destroy(&mut attr);
        let lo = addr as usize + guard + 0x10000; // stay above the guard page with margin
        (lo, addr as usize + size)
    }
}

/// Build a TEB for the calling thread and point GS at it. Returns the TEB address.
pub fn setup_current_thread() -> usize {
    let existing = TEB.with(|t| t.get());
    if existing != 0 {
        return existing;
    }
    let teb = zalloc(TEB_SIZE);
    let tpl = TLS_TEMPLATE.get().expect("TLS template");
    let block_len = (tpl.len + tpl.zero_fill).max(0x100);
    let block = zalloc((block_len + 0xfff) & !0xfff);
    unsafe { std::ptr::copy_nonoverlapping(tpl.start as *const u8, block as *mut u8, tpl.len) };
    let tls_array = zalloc(0x1000);
    let expansion = zalloc(TLS_EXPANSION * 8);
    let (lo, hi) = current_stack();
    unsafe {
        *(tls_array as *mut u64) = block as u64;
        let w = |off: usize, v: u64| *((teb + off) as *mut u64) = v;
        w(TEB_STACK_BASE, hi as u64);
        w(TEB_STACK_LIMIT, lo as u64);
        w(TEB_SELF, teb as u64);
        w(TEB_PID, libc::getpid() as u64);
        w(TEB_TID, gettid());
        w(TEB_TLS_PTR, tls_array as u64);
        w(TEB_PEB, peb() as u64);
        w(TEB_TLS_EXPANSION, expansion as u64);
        let r = libc::syscall(libc::SYS_arch_prctl, 0x1001 /* ARCH_SET_GS */, teb as u64);
        assert_eq!(r, 0, "arch_prctl(ARCH_SET_GS) failed");
        // Windows x64 starts threads with x87 control word 0x27F (53-bit precision); Linux uses 0x37F.
        let cw: u16 = 0x27F;
        std::arch::asm!("fldcw [{0}]", in(reg) &cw, options(nostack));
    }
    crate::crash::install_altstack();
    crate::crash::TIDS.lock().unwrap().push(gettid() as i32);
    TEB.with(|t| t.set(teb));
    teb
}

pub fn current() -> usize {
    let t = TEB.with(|t| t.get());
    assert!(t != 0, "thread has no TEB");
    t
}

pub fn last_error() -> u32 {
    unsafe { *((current() + TEB_LAST_ERROR) as *const u32) }
}
pub fn set_last_error(e: u32) {
    let t = TEB.with(|t| t.get());
    if t != 0 {
        unsafe { *((t + TEB_LAST_ERROR) as *mut u32) = e };
    }
}

pub fn tls_get(i: usize) -> u64 {
    let t = current();
    unsafe {
        if i < TLS_SLOTS {
            *((t + TEB_TLS_SLOTS + i * 8) as *const u64)
        } else {
            let e = *((t + TEB_TLS_EXPANSION) as *const u64) as usize;
            *((e + (i - TLS_SLOTS) * 8) as *const u64)
        }
    }
}
pub fn tls_set(i: usize, v: u64) {
    let t = current();
    unsafe {
        if i < TLS_SLOTS {
            *((t + TEB_TLS_SLOTS + i * 8) as *mut u64) = v;
        } else {
            let e = *((t + TEB_TLS_EXPANSION) as *const u64) as usize;
            *((e + (i - TLS_SLOTS) * 8) as *mut u64) = v;
        }
    }
}

/// Run the image's TLS callbacks for this thread (reason 2 = DLL_THREAD_ATTACH, as the CRT does
/// for the main thread through __dyn_tls_init 0x2006DC0F8).
pub fn run_tls_callbacks(reason: u32) {
    let tpl = TLS_TEMPLATE.get().unwrap();
    for &cb in &tpl.callbacks {
        let f: extern "win64" fn(u64, u32, u64) = unsafe { std::mem::transmute(cb) };
        f(IMAGE_BASE.load(Ordering::Relaxed), reason, 0);
    }
}
