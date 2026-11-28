//! Fault reporting: SIGSEGV/SIGBUS/SIGILL/SIGFPE/SIGTRAP print RIP, registers and the image return
//! addresses found on the stack, then end the process. Runs on an alternate stack per thread.

use std::fmt::Write as _;

static mut TEXT_LO: usize = 0;
static mut TEXT_HI: usize = 0;

pub fn set_text_range(lo: usize, hi: usize) {
    unsafe {
        TEXT_LO = lo;
        TEXT_HI = hi;
    }
}

extern "C" fn handler(sig: libc::c_int, info: *mut libc::siginfo_t, ctx: *mut libc::c_void) {
    unsafe {
        let uc = &*(ctx as *const libc::ucontext_t);
        let g = &uc.uc_mcontext.gregs;
        // a host hardware breakpoint (bp.rs): SIGTRAP with si_code TRAP_PERF (6); record and resume
        if sig == libc::SIGTRAP && (*info).si_code == 6 {
            // si_perf_data follows si_addr in the sigfault union (siginfo + 0x18)
            crate::bp::on_trap(g, *((info as *const u8).add(0x18) as *const u64), ctx as *const libc::ucontext_t);
            return;
        }
        let addr = (*info).si_addr() as usize;
        // HOST_NULL_RES_GUARD=right (diagnosis): an access to the page after a guarded empty resource names its class
        if sig == libc::SIGSEGV
            && let Some(n) = crate::nullcook::guard_fault_note(addr, g[libc::REG_RIP as usize] as usize)
        {
            crate::log::raw(format!("{n}\n").as_bytes());
            crate::log::line(&n);
        }
        let s = describe(&format!("FAULT: signal {sig} at rip {:#x} (fault address {addr:#x})", g[libc::REG_RIP as usize] as usize), g, 24);
        crate::log::raw(s.as_bytes());
        // best effort: also into the log file
        crate::log::line(&s);
        // a fault inside k32::call_fault_guarded: resume at the guard's exit instead of ending the run
        if sig == libc::SIGSEGV || sig == libc::SIGBUS {
            if let Some((ip, rdi, rsi)) = crate::k32::fault_escape() {
                crate::log::line("  -> the fault-guarded call is left (the exe frames in between are abandoned)");
                crate::log::flush();
                let uc = &mut *(ctx as *mut libc::ucontext_t);
                uc.uc_mcontext.gregs[libc::REG_RIP as usize] = ip as i64;
                uc.uc_mcontext.gregs[libc::REG_RDI as usize] = rdi as i64;
                uc.uc_mcontext.gregs[libc::REG_RSI as usize] = rsi as i64;
                return;
            }
        }
        crate::log::flush();
        crate::run::write_stub_report();
        libc::_exit(5);
    }
}

/// Registers, the image return addresses on the stack (up to `max_ret`) and the HOST_FAULT_DUMP regions of a thread
/// state, as one text block (used by the fault handler and by bp.rs breakpoint hits).
pub unsafe fn describe(title: &str, g: &[libc::greg_t], max_ret: usize) -> String {
    unsafe {
        let rip = g[libc::REG_RIP as usize] as usize;
        let rsp = g[libc::REG_RSP as usize] as usize;
        let mut s = String::with_capacity(4096);
        let _ = writeln!(s, "{title}");
        let _ = rip;
        let names = [
            ("rax", libc::REG_RAX), ("rbx", libc::REG_RBX), ("rcx", libc::REG_RCX), ("rdx", libc::REG_RDX),
            ("rsi", libc::REG_RSI), ("rdi", libc::REG_RDI), ("rbp", libc::REG_RBP), ("rsp", libc::REG_RSP),
            ("r8", libc::REG_R8), ("r9", libc::REG_R9), ("r10", libc::REG_R10), ("r11", libc::REG_R11),
            ("r12", libc::REG_R12), ("r13", libc::REG_R13), ("r14", libc::REG_R14), ("r15", libc::REG_R15),
        ];
        for (i, (n, r)) in names.iter().enumerate() {
            let _ = write!(s, "  {n}={:#018x}", g[*r as usize] as u64);
            if i % 4 == 3 {
                s.push('\n');
            }
        }
        let (lo, hi) = (TEXT_LO, TEXT_HI);
        let _ = write!(s, "  image return addresses on the stack:");
        let mut n = 0;
        let mut p = rsp;
        while p < rsp + 0x4000 && n < max_ret {
            // only read the current thread's stack page range that is mapped (stack is fully mapped)
            let v = *(p as *const usize);
            if v >= lo && v < hi {
                let _ = write!(s, " {v:#x}@+{:#x}", p - rsp);
                n += 1;
            }
            p += 8;
        }
        s.push('\n');
        // HOST_FAULT_DUMP="r13+0x70:0x360[,...]": hex dump of memory at register + offset (diagnosis only)
        if let Ok(spec) = std::env::var("HOST_FAULT_DUMP") {
            for part in spec.split(',') {
                let Some((loc, len)) = part.split_once(':') else { continue };
                // a leading '*' dumps at the qword stored at register + offset
                let (deref, loc) = match loc.strip_prefix('*') {
                    Some(l) => (true, l),
                    None => (false, loc),
                };
                let (reg, off) = loc.split_once('+').unwrap_or((loc, "0"));
                let pnum = |x: &str| u64::from_str_radix(x.trim_start_matches("0x"), 16).unwrap_or(0);
                let Some((_, r)) = names.iter().find(|(n, _)| *n == reg) else { continue };
                let mut base = (g[*r as usize] as u64).wrapping_add(pnum(off));
                if deref {
                    base = *(base as *const u64);
                    if base < 0x10000 || base >= 0x1000_0000_0000 {
                        let _ = writeln!(s, "  dump {part}: pointer {base:#x} skipped");
                        continue;
                    }
                }
                let len = pnum(len).min(0x2000);
                let _ = writeln!(s, "  dump {part} = {base:#x}:");
                let mut a = base;
                while a < base + len {
                    let row = std::slice::from_raw_parts(a as *const u8, 16);
                    let txt: String = row.iter().map(|&b| if (0x20..0x7f).contains(&b) { b as char } else { '.' }).collect();
                    let _ = writeln!(s, "    +{:#05x} {:02x?} {txt}", a - base, row);
                    a += 16;
                }
            }
        }
        s
    }
}

/// Thread ids of every thread that has a TEB (exe threads + the world thread).
pub static TIDS: std::sync::Mutex<Vec<i32>> = std::sync::Mutex::new(Vec::new());

/// SIGUSR2 (sent to the process): forward SIGUSR1 to every exe thread.
extern "C" fn dump_all(_sig: libc::c_int) {
    if let Ok(v) = TIDS.try_lock() {
        for &t in v.iter() {
            unsafe { libc::syscall(libc::SYS_tgkill, libc::getpid(), t, libc::SIGUSR1) };
        }
    }
}

/// SIGUSR1: print this thread's instruction pointer and the image return addresses on its stack (non-fatal).
extern "C" fn dump_one(_sig: libc::c_int, _info: *mut libc::siginfo_t, ctx: *mut libc::c_void) {
    unsafe {
        let uc = &*(ctx as *const libc::ucontext_t);
        let g = &uc.uc_mcontext.gregs;
        let rip = g[libc::REG_RIP as usize] as usize;
        let rsp = g[libc::REG_RSP as usize] as usize;
        let (lo, hi) = (TEXT_LO, TEXT_HI);
        let mut s = String::with_capacity(2048);
        let _ = write!(s, "THREAD DUMP tid {}: rip {rip:#x} rsp {rsp:#x}; image return addresses:", libc::syscall(libc::SYS_gettid));
        let mut n = 0;
        let mut p = rsp;
        while p < rsp + 0x8000 && n < 20 {
            let v = *(p as *const usize);
            if v >= lo && v < hi {
                let _ = write!(s, " {v:#x}");
                n += 1;
            }
            p += 8;
        }
        crate::log::line(&s);
        crate::log::flush();
    }
}

pub fn install() {
    unsafe {
        let mut sa: libc::sigaction = std::mem::zeroed();
        sa.sa_sigaction = dump_one as usize;
        sa.sa_flags = libc::SA_SIGINFO | libc::SA_ONSTACK | libc::SA_RESTART;
        libc::sigemptyset(&mut sa.sa_mask);
        libc::sigaction(libc::SIGUSR1, &sa, std::ptr::null_mut());
        let mut sb: libc::sigaction = std::mem::zeroed();
        sb.sa_sigaction = dump_all as usize;
        sb.sa_flags = libc::SA_RESTART;
        libc::sigemptyset(&mut sb.sa_mask);
        libc::sigaction(libc::SIGUSR2, &sb, std::ptr::null_mut());
    }
    unsafe {
        for sig in [libc::SIGSEGV, libc::SIGBUS, libc::SIGILL, libc::SIGFPE, libc::SIGTRAP] {
            let mut sa: libc::sigaction = std::mem::zeroed();
            sa.sa_sigaction = handler as usize;
            sa.sa_flags = libc::SA_SIGINFO | libc::SA_ONSTACK;
            libc::sigemptyset(&mut sa.sa_mask);
            libc::sigaction(sig, &sa, std::ptr::null_mut());
        }
    }
    install_altstack();
}

pub fn install_altstack() {
    unsafe {
        let size = 0x40000;
        let p = libc::mmap(std::ptr::null_mut(), size, libc::PROT_READ | libc::PROT_WRITE, libc::MAP_PRIVATE | libc::MAP_ANONYMOUS, -1, 0);
        let ss = libc::stack_t { ss_sp: p, ss_flags: 0, ss_size: size };
        libc::sigaltstack(&ss, std::ptr::null_mut());
    }
}
