//! Loader + the one-shot research steps.
//!
//! Step 1: map the PE at 0x200000000, fill all import tables, fake TEB, run the CRT start-up,
//!         the 8 `__xi` initialisers and the 777 `__xc` static constructors (never WinMain),
//!         set the mt_* bytes to 0, then command line + engine_world ctor (memory arenas).
//! Step 2/3: see `step2` / `step3`.

use crate::files::{CFG, FsConfig};
use crate::hlog;
use crate::k32::{PROC, ProcCfg};
use crate::mem;
use crate::pe::Pe;
use crate::teb;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

static VERBOSE_STUBS: AtomicBool = AtomicBool::new(true);
static VERBOSE_FILES: AtomicBool = AtomicBool::new(true);
static VERBOSE_MEM: AtomicBool = AtomicBool::new(false);
static OUT: OnceLock<PathBuf> = OnceLock::new();
/// Imports (DLL!name or name) at which a guarded exe call is left (see k32::escape).
static ESCAPE_AT: OnceLock<Vec<String>> = OnceLock::new();
/// Exe thread names (":sound", ":network") ended at their naming call.
static END_THREADS: OnceLock<Vec<String>> = OnceLock::new();

static END_AT: OnceLock<Vec<String>> = OnceLock::new();

/// `--end-at` entries: "DLL!name", "name" or "DLL" (any import of that DLL).
pub fn is_end_at(dll: &str, name: &str) -> bool {
    let Some(v) = END_AT.get() else { return false };
    let full = format!("{dll}!{name}");
    v.iter().any(|e| e.eq_ignore_ascii_case(&full) || e.eq_ignore_ascii_case(name) || e.eq_ignore_ascii_case(dll))
}

pub fn is_ended_thread(name: &str) -> bool {
    END_THREADS.get().map(|v| v.iter().any(|x| x == name)).unwrap_or(false)
}

pub fn is_escape_point(dll: &str, name: &str) -> bool {
    let Some(v) = ESCAPE_AT.get() else { return false };
    let full = format!("{dll}!{name}");
    v.iter().any(|e| e.eq_ignore_ascii_case(&full) || e.eq_ignore_ascii_case(name))
}

pub fn verbose_stubs() -> bool {
    VERBOSE_STUBS.load(Ordering::Relaxed)
}
pub fn verbose_files() -> bool {
    VERBOSE_FILES.load(Ordering::Relaxed)
}
pub fn verbose_mem() -> bool {
    VERBOSE_MEM.load(Ordering::Relaxed)
}

/// Every import that was called, with counts; also written to <out>/stubs.txt.
pub fn write_stub_report() {
    let rep = crate::thunks::report();
    let mut s = String::new();
    for (dll, name, n, imp, first) in &rep {
        s.push_str(&format!("{dll}!{name}\tcalls={n}\t{}\tfirst_from={first:#x}\n", if *imp { "host" } else { "TRAP" }));
    }
    if let Some(o) = OUT.get() {
        let _ = std::fs::write(o.join("stubs.txt"), &s);
    }
}

pub fn abort_run(code: i32) -> ! {
    hlog!("run ends with code {code}");
    write_stub_report();
    crate::log::flush();
    unsafe { libc::_exit(code) }
}

pub fn mem_snapshot(label: &str) {
    let st = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let pick = |k: &str| st.lines().find(|l| l.starts_with(k)).map(|l| l.split_whitespace().nth(1).unwrap_or("?").to_string()).unwrap_or("?".into());
    hlog!(
        "MEM [{label}]: VmSize {} kB, VmRSS {} kB (RssAnon {} kB, RssFile {} kB, RssShmem {} kB), VmHWM {} kB",
        pick("VmSize:"),
        pick("VmRSS:"),
        pick("RssAnon:"),
        pick("RssFile:"),
        pick("RssShmem:"),
        pick("VmHWM:")
    );
    if let Ok(r) = std::fs::read_to_string("/proc/self/smaps_rollup") {
        let v: Vec<String> = r
            .lines()
            .filter(|l| ["Rss:", "Pss:", "Shared_Clean:", "Shared_Dirty:", "Private_Clean:", "Private_Dirty:", "Anonymous:"].iter().any(|k| l.starts_with(k)))
            .map(|l| l.split_whitespace().take(2).collect::<Vec<_>>().join(" "))
            .collect();
        hlog!("MEM [{label}] smaps_rollup: {}", v.join(", "));
    }
}

/// Where the private dirty memory is: per-region totals from /proc/self/smaps, plus the resident
/// 1 MiB buckets of the image's .data/BSS (mincore), to attribute it to exe globals.
pub fn mem_breakdown(label: &str) {
    let Ok(sm) = std::fs::read_to_string("/proc/self/smaps") else { return };
    let mut cur: Option<(usize, usize, String)> = None;
    let mut sums: std::collections::BTreeMap<String, (u64, u64)> = std::collections::BTreeMap::new();
    let classify = |a: usize, b: usize, name: &str| -> String {
        if a >= 0x200000000 && b <= 0x215E38000 {
            return match a {
                0x200a90000 => "image .data+BSS".into(),
                0x200001000 => "image .text".into(),
                _ => "image other".into(),
            };
        }
        if a >= mem::LOW_START && b <= mem::LOW_END {
            let sz = b - a;
            return if sz >= (1usize << 30) { "low: VirtualAlloc arena block (>=1 GiB)".into() } else { format!("low: other ({})", if sz >= 64 << 20 { "heap chunk/stack/arena" } else { "small" }) };
        }
        if name.contains("world-host") { return "host binary".into() }
        if name.starts_with('[') { return name.to_string() }
        if name.is_empty() { return "host anon (Rust heap, thread stacks)".into() }
        "host libs".into()
    };
    let mut rss = 0u64;
    let mut pd = 0u64;
    let flush = |cur: &Option<(usize, usize, String)>, rss: u64, pd: u64, sums: &mut std::collections::BTreeMap<String, (u64, u64)>| {
        if let Some((a, b, n)) = cur {
            let e = sums.entry(classify(*a, *b, n)).or_default();
            e.0 += rss;
            e.1 += pd;
        }
    };
    for l in sm.lines() {
        let first = l.split_whitespace().next().unwrap_or("");
        if first.contains('-') && !first.ends_with(':') {
            flush(&cur, rss, pd, &mut sums);
            rss = 0;
            pd = 0;
            let (a, b) = first.split_once('-').unwrap();
            let name = l.split_whitespace().nth(5).unwrap_or("").to_string();
            cur = Some((usize::from_str_radix(a, 16).unwrap_or(0), usize::from_str_radix(b, 16).unwrap_or(0), name));
        } else if let Some(v) = l.strip_prefix("Rss:") {
            rss = v.trim().trim_end_matches(" kB").trim().parse().unwrap_or(0);
        } else if let Some(v) = l.strip_prefix("Private_Dirty:") {
            pd = v.trim().trim_end_matches(" kB").trim().parse().unwrap_or(0);
        }
    }
    flush(&cur, rss, pd, &mut sums);
    for (k, (r, d)) in &sums {
        if *r > 512 {
            hlog!("MEMBREAK [{label}] {k}: Rss {:.1} MB, Private_Dirty {:.1} MB", *r as f64 / 1024.0, *d as f64 / 1024.0);
        }
    }
    // resident 1 MiB buckets of .data/BSS
    let (lo, hi) = (0x200a90000usize, 0x215d9e000usize);
    let pages = (hi - lo) / 4096;
    let mut vec = vec![0u8; pages];
    if unsafe { libc::mincore(lo as *mut libc::c_void, hi - lo, vec.as_mut_ptr()) } == 0 {
        let mut buckets: Vec<(usize, usize)> = Vec::new();
        for (i, chunk) in vec.chunks(256).enumerate() {
            let n = chunk.iter().filter(|&&x| x & 1 != 0).count();
            if n > 0 {
                buckets.push((lo + i * (1 << 20), n));
            }
        }
        let total: usize = buckets.iter().map(|b| b.1).sum();
        let mut runs: Vec<(usize, usize, usize)> = Vec::new(); // start, end, pages
        for &(a, n) in &buckets {
            match runs.last_mut() {
                Some(r) if r.1 == a => {
                    r.1 = a + (1 << 20);
                    r.2 += n;
                }
                _ => runs.push((a, a + (1 << 20), n)),
            }
        }
        let rl: Vec<String> = runs.iter().filter(|r| r.2 >= 128).map(|r| format!("{:#x}..{:#x}:{:.1}MB", r.0, r.1, r.2 as f64 * 4.0 / 1024.0)).collect();
        hlog!("MEMBREAK [{label}] .data/BSS resident runs (>=0.5 MB): {}", rl.join(" "));
        let mut top = buckets.clone();
        top.sort_by(|a, b| b.1.cmp(&a.1));
        let list: Vec<String> = top.iter().take(12).map(|(a, n)| format!("{a:#x}:{:.1}MB", *n as f64 * 4.0 / 1024.0)).collect();
        hlog!("MEMBREAK [{label}] .data/BSS resident {:.1} MB in {} MiB-buckets; top: {}", total as f64 * 4.0 / 1024.0, buckets.len(), list.join(" "));
    }
}

// ------------------------------------------------------------------ loader

pub struct Loaded {
    pub pe: Pe,
    pub file: Vec<u8>,
}

fn sec_protect(ch: u32) -> u32 {
    let x = ch & 0x2000_0000 != 0;
    let w = ch & 0x8000_0000 != 0;
    match (x, w) {
        (true, true) => mem::PAGE_EXECUTE_READWRITE,
        (true, false) => mem::PAGE_EXECUTE_READ,
        (false, true) => mem::PAGE_READWRITE,
        (false, false) => mem::PAGE_READONLY,
    }
}

/// vostok_scaleform (the UI DLL) is not in the host: its constructors are logged no-ops returning `this`,
/// destructors logged no-ops; any other method still traps so each use is visible.
unsafe extern "win64" fn scaleform_ctor(this: u64) -> u64 {
    hlog!("vostok_scaleform constructor (no-op) this={this:#x} from {:#x}", crate::thunks::caller());
    this
}
unsafe extern "win64" fn scaleform_dtor(this: u64) -> u64 {
    hlog!("vostok_scaleform destructor (no-op) this={this:#x}");
    0
}

/// flash_value setters (`?Set...@flash_value@survarium@@QEAAX...`, void): UI values for a HUD the host does not have.
/// The match tick reaches them (e.g. SetString from 0x2001A2F8B inside the world step); logged no-ops.
unsafe extern "win64" fn scaleform_set(this: u64) -> u64 {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    if N.fetch_add(1, Ordering::Relaxed) < 20 {
        hlog!("vostok_scaleform flash_value setter (no-op) this={this:#x} from {:#x}", crate::thunks::caller());
    }
    0
}

fn lookup_impl(dll: &str, name: &str) -> usize {
    let d = dll.to_ascii_lowercase();
    if d == "vostok_scaleform.dll" && name.starts_with("?Set") && name.contains("@flash_value@survarium@@QEAAX") {
        return scaleform_set as *const () as usize;
    }
    // flash_movie methods (Invoke etc.): the host's HUD movie is a null object; calls answer 0 (false)
    if d == "vostok_scaleform.dll" && name.contains("@flash_movie@survarium@@") {
        return scaleform_set as *const () as usize;
    }
    if d == "vostok_scaleform.dll" && name.starts_with("??0") {
        return scaleform_ctor as *const () as usize;
    }
    if d == "vostok_scaleform.dll" && name.starts_with("??1") {
        return scaleform_dtor as *const () as usize;
    }
    if d == "kernel32.dll" {
        return crate::k32::table().into_iter().find(|(n, _)| *n == name).map(|(_, a)| a).unwrap_or(0);
    }
    crate::other::table()
        .into_iter()
        .chain(crate::crypt::table())
        .chain(crate::dinput::table())
        .chain(crate::d3d::table_entries())
        .find(|(dl, n, _)| *dl == d && *n == name)
        .map(|(_, _, a)| a)
        .unwrap_or(0)
}

pub fn load(path: &str) -> Result<Loaded, String> {
    let file = std::fs::read(path).map_err(|e| format!("read {path}: {e}"))?;
    let pe = Pe::parse(&file)?;
    let base = pe.image_base as usize;
    let size = pe.size_of_image as usize;
    hlog!("PE: base {base:#x} size {size:#x} entry {:#x} sections {} imports {}", base + pe.entry_rva as usize, pe.sections.len(), pe.imports.len());
    // 0x200000000 is the exe's fixed base (no DYNAMIC_BASE); never relocate.
    let p = unsafe {
        libc::mmap(
            base as *mut libc::c_void,
            size,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_FIXED_NOREPLACE | libc::MAP_NORESERVE,
            -1,
            0,
        )
    };
    if p as usize != base {
        return Err(format!("cannot map image at {base:#x}"));
    }
    unsafe {
        std::ptr::copy_nonoverlapping(file.as_ptr(), base as *mut u8, pe.size_of_headers as usize);
        for s in &pe.sections {
            let n = s.raw_size.min(s.vsize.max(s.raw_size)) as usize;
            let n = n.min(file.len() - s.raw_off as usize);
            std::ptr::copy_nonoverlapping(file.as_ptr().add(s.raw_off as usize), (base + s.va as usize) as *mut u8, n);
        }
    }
    // imports: every slot -> thunk (host implementation or trap)
    let (mut n_impl, mut n_trap) = (0, 0);
    for imp in &pe.imports {
        let a = lookup_impl(&imp.dll, &imp.name);
        if a != 0 {
            n_impl += 1;
        } else {
            n_trap += 1;
        }
        let th = crate::thunks::make(&imp.dll, &imp.name, a);
        unsafe { *((base + imp.slot_rva as usize) as *mut u64) = th as u64 };
    }
    let n_delay = pe.imports.iter().filter(|i| i.delay).count();
    let n_dlls: std::collections::BTreeSet<_> = pe.imports.iter().filter(|i| i.delay).map(|i| i.dll.clone()).collect();
    hlog!(
        "imports: {} slots ({} static, {} delay in {} DLLs); {} with host stubs, {} trap",
        pe.imports.len(),
        pe.imports.len() - n_delay,
        n_delay,
        n_dlls.len(),
        n_impl,
        n_trap
    );
    // TLS: _tls_index = 0 (428 sites hard-wire slot 0)
    let tls = pe.tls.clone().ok_or("no TLS directory")?;
    unsafe { *(tls.index_va as *mut u32) = 0 };
    let mut cbs = Vec::new();
    let mut c = tls.callbacks_va;
    loop {
        let v = unsafe { *(c as *const u64) };
        if v == 0 {
            break;
        }
        cbs.push(v as usize);
        c += 8;
    }
    hlog!("TLS: template {:#x}..{:#x} ({} bytes), index {:#x} := 0, callbacks {:x?}", tls.start, tls.end, tls.end - tls.start, tls.index_va, cbs);
    let _ = teb::TLS_TEMPLATE.set(teb::TlsTemplate {
        start: tls.start as usize,
        len: (tls.end - tls.start) as usize,
        zero_fill: tls.zero_fill as usize,
        callbacks: cbs,
    });
    teb::IMAGE_BASE.store(base as u64, Ordering::Relaxed);
    // protections + VirtualQuery map
    let mut runs: Vec<(usize, usize, u32)> = vec![(base, base + 0x1000, mem::PAGE_READONLY)];
    for s in &pe.sections {
        let st = base + s.va as usize;
        let en = base + ((s.va as usize + s.vsize as usize + 0xfff) & !0xfff);
        let pr = sec_protect(s.characteristics);
        unsafe { libc::mprotect(st as *mut libc::c_void, en - st, mem::win_to_prot(pr)) };
        runs.push((st, en, pr));
    }
    unsafe { libc::mprotect(base as *mut libc::c_void, 0x1000, libc::PROT_READ) };
    let mut merged: Vec<(usize, usize, u32)> = Vec::new();
    for r in runs {
        if let Some(l) = merged.last_mut() {
            if l.1 == r.0 && l.2 == r.2 {
                l.1 = r.1;
                continue;
            }
        }
        merged.push(r);
    }
    for r in &merged {
        hlog!("image run {:#x}..{:#x} protect {:#x}", r.0, r.1, r.2);
    }
    mem::set_image_map(mem::ImageMap { base, size, runs: merged });
    let text = pe.sections.iter().find(|s| s.name == ".text").ok_or("no .text")?;
    crate::crash::set_text_range(base + text.va as usize, base + (text.va + text.vsize) as usize);
    Ok(Loaded { pe, file })
}

/// .text in memory == .text in the file.
pub fn text_matches(l: &Loaded) -> bool {
    let base = l.pe.image_base as usize;
    let t = l.pe.sections.iter().find(|s| s.name == ".text").unwrap();
    let n = t.vsize.min(t.raw_size) as usize;
    let mem = unsafe { std::slice::from_raw_parts((base + t.va as usize) as *const u8, n) };
    mem == &l.file[t.raw_off as usize..t.raw_off as usize + n]
}

// ------------------------------------------------------------------ calling exe code

pub unsafe fn call0(a: u64) -> u64 {
    let f: extern "win64" fn() -> u64 = unsafe { std::mem::transmute(a) };
    f()
}
pub unsafe fn call1(a: u64, x: u64) -> u64 {
    let f: extern "win64" fn(u64) -> u64 = unsafe { std::mem::transmute(a) };
    f(x)
}
pub unsafe fn call2(a: u64, x: u64, y: u64) -> u64 {
    let f: extern "win64" fn(u64, u64) -> u64 = unsafe { std::mem::transmute(a) };
    f(x, y)
}
pub unsafe fn call3(a: u64, x: u64, y: u64, z: u64) -> u64 {
    let f: extern "win64" fn(u64, u64, u64) -> u64 = unsafe { std::mem::transmute(a) };
    f(x, y, z)
}
pub unsafe fn call4(a: u64, x: u64, y: u64, z: u64, w: u64) -> u64 {
    let f: extern "win64" fn(u64, u64, u64, u64) -> u64 = unsafe { std::mem::transmute(a) };
    f(x, y, z, w)
}
pub unsafe fn rd64(a: u64) -> u64 {
    unsafe { *(a as *const u64) }
}
pub unsafe fn rd8(a: u64) -> u8 {
    unsafe { *(a as *const u8) }
}


// ------------------------------------------------------------------ resource queries (exe API)
//
// 0x2000A9EF0 query_resource_and_wait(path, class_id, callback functor, allocator, 0, 0) runs a query and pumps the
// callback dispatch on the calling thread until it is done; the allocator is the global at 0x20F43E400.
// queries_result: entries at +0x70 + i*0x360, the resource pointer at entry+0x1C8.
// binary_config: root value at resource+0x208; operator[](const char*) = 0x2000CC0E0; a value's type is in bits
// 44..47 (2 = float in the low 32 bits).
pub const CLASS_BINARY_CONFIG: u64 = 0x1F;

static CB_RESULT: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

unsafe extern "win64" fn functor_manager(_a: u64, _b: u64, _op: u64) -> u64 {
    0
}

pub unsafe fn cfg_get(v: u64, key: &str) -> u64 {
    let k = crate::strings::leak_cstr(key);
    unsafe { call2(0x2000CC0E0, v, k) }
}
pub unsafe fn cfg_raw(v: u64) -> u64 {
    unsafe { rd64(v) }
}

unsafe extern "win64" fn config_query_cb(_buf: u64, result: u64) -> u64 {
    let mut out = Vec::new();
    unsafe {
        let entry = result + 0x70;
        let res = rd64(entry + 0x1C8);
        out.push(format!("callback: queries_result {result:#x}, entry flags {:#x}, resource {res:#x}", *((entry + 0x340) as *const u32)));
        if res != 0 {
            let root = rd64(res + 0x208);
            out.push(format!("binary_config root value {root:#x}"));
            let params = cfg_get(root, "parameters");
            for k in ["first_warning_time_seconds", "give_fine_time_seconds"] {
                let v = cfg_get(params, k);
                let raw = cfg_raw(v);
                out.push(format!("parameters.{k}: raw {raw:#018x} type {} value {}", (raw >> 44) & 0xF, raw as u32));
            }
            let charges = cfg_get(params, "charges");
            let m = cfg_raw(cfg_get(charges, "money"));
            out.push(format!("parameters.charges.money: raw {m:#018x} type {} value {}", (m >> 44) & 0xF, m as u32));
        }
    }
    for l in &out {
        hlog!("{l}");
    }
    *CB_RESULT.lock().unwrap() = out;
    0
}

/// Query one binary config through the exe and read three values from it.
pub fn query_config(path_va: u64) -> Vec<String> {
    unsafe {
        let vt = mem::heap_alloc(16, true) as u64;
        *(vt as *mut u64) = functor_manager as *const () as u64;
        *((vt + 8) as *mut u64) = config_query_cb as *const () as u64;
        let functor = mem::heap_alloc(0x30, true) as u64;
        *(functor as *mut u64) = vt | 1; // trivially copyable functor (bit 0), as 0x2002DADFA does
        let alloc = rd64(0x20F43E400);
        hlog!("query_resource_and_wait({}, class {CLASS_BINARY_CONFIG:#x}), allocator {alloc:#x}", crate::strings::cstr(path_va));
        let f: extern "win64" fn(u64, u64, u64, u64, u64, u64) -> u64 = std::mem::transmute(0x2000A9EF0u64);
        f(path_va, CLASS_BINARY_CONFIG, functor, alloc, 0, 0);
    }
    CB_RESULT.lock().unwrap().clone()
}

// ------------------------------------------------------------------ steps

/// mt_* console-variable value bytes (all 1 in the file).
pub const MT_BYTES: [(&str, u64); 9] = [
    ("mt_local_joints", 0x200AA342A),
    ("mt_local_joints_to_object", 0x200AA3428),
    ("mt_character_controllers", 0x200AA3098),
    ("mt_recompute_damage_collision", 0x200AA3148),
    ("mt_tick_players_before_physics", 0x200AA30E7),
    ("mt_tick_players_after_physics", 0x200AA30E6),
    ("mt_serialize_to_buffer", 0x200AA30E8),
    ("mt_serialize", 0x200AA30E4),
    ("mt_deserialize", 0x200AA30E5),
];
pub const FMA3_FLAG: u64 = 0x200C2CCA4;

pub struct Opts {
    pub exe: String,
    pub exe_win: String,
    pub out: PathBuf,
    pub steps: u32,
    pub fma: Option<u32>,
    pub level: String,
    pub cmdline_extra: String,
    pub linger_ms: Option<u64>,
    pub pump_ms: u64,
}

fn step1(l: &Loaded, o: &Opts) -> Result<(), String> {
    hlog!("=== step 1: CRT start-up, static constructors, memory init");
    if !text_matches(l) {
        return Err(".text differs from the file after mapping".into());
    }
    hlog!("check: .text bytes equal the file (after mapping and import fill)");
    teb::setup_current_thread();
    unsafe {
        // the exe's CRT entry 0x2006DBAD8 minus WinMain: security cookie, CRT init, _initterm_e(__xi),
        // _initterm(__xc), the TLS callback
        call0(0x2006DC4A0);
        hlog!("__security_init_cookie done (cookie {:#x})", rd64(0x200A90118));
        let ok = call1(0x2006DBB28, 1) & 0xff;
        hlog!("__scrt_initialize_crt(1) -> {ok}");
        if ok == 0 {
            return Err("__scrt_initialize_crt failed".into());
        }
        // __scrt_current_native_startup_state := initializing (as 0x2006DB9A2 does)
        let r = call2(0x200719D70, 0x2008A8030, 0x2008A8078) as u32;
        hlog!("_initterm_e(__xi: 8 initialisers) -> {r}");
        if r != 0 {
            return Err(format!("_initterm_e failed: {r}"));
        }
        hlog!("FMA3/AVX dispatch flag [{FMA3_FLAG:#x}] after __xi = {:#x}", *(FMA3_FLAG as *const u32));
        call2(0x200719D0C, 0x2008A67C0, 0x2008A8010);
        hlog!("_initterm(__xc: 777 static constructors) done");
        teb::run_tls_callbacks(2);
        hlog!("TLS callback (thread attach) done");
    }
    // start-up checks: the cvar list is filled and a known static object has its vtable
    let cvar_head = unsafe { rd64(0x20F43C638) };
    let vt = unsafe { rd64(0x200AB7930) };
    hlog!("check: cvar list head [0x20F43C638] = {cvar_head:#x} ({})", if cvar_head != 0 { "filled" } else { "EMPTY" });
    hlog!("check: [0x200AB7930] vtable = {vt:#x} ({})", if vt == 0x200927200 { "ok" } else { "MISMATCH" });
    if cvar_head == 0 || vt != 0x200927200 {
        return Err("start-up checks failed".into());
    }
    if !text_matches(l) {
        return Err(".text changed during start-up".into());
    }
    hlog!("check: .text still equals the file after the constructors");
    // HOST_MT=all | name,name: keep those mt_* at the file's value 1 (the client's), float drift diagnosis
    let keep = std::env::var("HOST_MT").unwrap_or_default();
    for (n, a) in MT_BYTES {
        let before = unsafe { rd8(a) };
        if keep == "all" || keep.split(',').any(|k| k == n) {
            hlog!("{n} [{a:#x}]: {before} (kept, HOST_MT)");
            continue;
        }
        unsafe { *(a as *mut u8) = 0 };
        hlog!("{n} [{a:#x}]: {before} -> 0");
    }
    let fma = unsafe { *(FMA3_FLAG as *const u32) };
    if let Some(v) = o.fma {
        unsafe { *(FMA3_FLAG as *mut u32) = v };
        hlog!("FMA3 flag pinned: {fma:#x} -> {v:#x}");
    } else {
        hlog!("FMA3 flag left as detected on this CPU: {fma:#x}");
    }
    mem_snapshot("after CRT + static ctors");
    mem_breakdown("after CRT + static ctors");
    // command line + engine_world ctor (memory arenas): 0x200088B30(app), as the exe's main reaches it
    let app = unsafe { rd64(0x200AA3910) };
    hlog!("application object [0x200AA3910] = {app:#x}");
    if app == 0 {
        return Err("application object pointer is null".into());
    }
    unsafe {
        // what the caller 0x200089160 sets first: the app object's fixed string buffer (begin, cur, end over
        // 0x200AA36F0..0x200AA38F0) and two globals
        *(0x200AA36F0 as *mut u8) = 0;
        *(0x200AA36D8 as *mut u64) = 0x200AA36F0;
        *(0x200AA36E0 as *mut u64) = 0x200AA36F0;
        *(0x200AA36E8 as *mut u64) = 0x200AA38F0;
        *(0x200AA3900 as *mut u64) = 0x200923A58;
        *(0x200AA3918 as *mut u32) = 1;
        // (the crash reporter's URL strings are skipped)
        *((app + 0x230) as *mut u32) = 0;
        call1(0x200088B30, app);
    }
    hlog!("0x200088B30 (command line, version check, build line, engine_world ctor) returned");
    mem_snapshot("after engine_world ctor");
    mem_breakdown("after engine_world ctor");
    Ok(())
}

/// Step 2: the client's own engine init 0x2000F5210(engine_world, on_initialized functor), run under a
/// guard and left (escape) at the first import of a subsystem a world host does not need.
fn step2(_l: &Loaded, o: &Opts) -> Result<(), String> {
    hlog!("=== step 2: core, file system, mounts, resource volumes (engine init 0x2000F5210, guarded)");
    let ew = unsafe { rd64(0x200AB16B0) };
    let app = unsafe { rd64(0x200AA3910) };
    hlog!("engine_world object [0x200AB16B0] = {ew:#x}");
    // the same functor the client passes: manager 0x200923720 (bit 0 set), fn 0x200088F70, app
    let mut functor: [u64; 8] = [0x200923721, 0x200088F70, app, 0, 0, 0, 0, 0];
    match crate::k32::call_guarded2(0x2000F5210, ew, functor.as_mut_ptr() as u64) {
        Ok(r) => hlog!("engine init 0x2000F5210 returned {r:#x}"),
        Err(c) => hlog!("engine init 0x2000F5210 left at the escape point (code {c:#x})"),
    }
    mem_snapshot("after engine init (step 2)");
    // The client's main loop delivers queued callbacks (among them the volume mount 0x2000F3DE0, which
    // enumerates resources.??.db): pump the dispatch 0x2000BCF60(&resources_manager, 0) on this thread.
    let t0 = std::time::Instant::now();
    let mut n = 0u64;
    while t0.elapsed() < std::time::Duration::from_millis(o.pump_ms) {
        unsafe { call2(0x2000BCF60, 0x200C70FE0, 0) };
        n += 1;
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    hlog!("pumped the callback dispatch {n} times ({} ms)", o.pump_ms);
    let (ok, bad) = crate::crypt::verify_counts();
    hlog!("volume FAT signatures (SHA-1 + RSA, key from the exe's certificate): {ok} valid, {bad} invalid");
    // one config query through the exe: resources/gameplay/fines/inactivity_fine.options (string 0x200987770)
    let r = query_config(0x200987770);
    let ok = r.iter().any(|l| l.contains("first_warning_time_seconds") && l.ends_with("value 180"))
        && r.iter().any(|l| l.contains("give_fine_time_seconds") && l.ends_with("value 300"));
    hlog!("check: config query returned the db values 180 / 300: {}", if ok { "PASS" } else { "FAIL" });
    mem_snapshot("after config query (step 2)");
    mem_breakdown("after config query (step 2)");
    if !ok {
        return Err("config query did not return the expected values".into());
    }
    Ok(())
}


static PROJ_RESULT: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
pub static GAME: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub static PROJECT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

unsafe extern "win64" fn project_query_cb(_buf: u64, result: u64) -> u64 {
    let mut out = Vec::new();
    unsafe {
        let entry = result + 0x70;
        let res = rd64(entry + 0x1C8);
        let flags = *((entry + 0x340) as *const u32);
        out.push(format!("project callback: entry flags {flags:#x}, resource {res:#x}"));
        if res != 0 {
            // The resource handle is simple_game_project+0x98 (resource-base vtable 0x20096FDD0 there); the project
            // holds the static geometry count at +0x50 and its records at +0x48.
            // Keep it (refcount +1 at resource+0x1B0, as the client does when it stores a resource); without it the
            // project is destroyed at the next pump and the physics allocator aborts.
            (*((res + 0x1B0) as *const std::sync::atomic::AtomicU32)).fetch_add(1, Ordering::SeqCst);
            let vt = rd64(res);
            let proj = res - 0x98;
            if vt == 0x20096FDD0 {
                let n = *((proj + 0x50) as *const u32);
                let t = *((proj + 0x90) as *const u32);
                PROJECT.store(proj, Ordering::Relaxed);
                out.push(format!("project resource {res:#x} (vtable {vt:#x}); simple_game_project {proj:#x} (vtable {:#x}): static geometry count [+0x50] = {n}, second list [+0x90] = {t}", rd64(proj)));
            } else {
                out.push(format!("project resource {res:#x} has vtable {vt:#x}, not the resource base 0x20096FDD0"));
            }
        }
    }
    for l in &out {
        hlog!("{l}");
    }
    *PROJ_RESULT.lock().unwrap() = out;
    0
}


// Typed request user data (vostok::resources user_data): +0x08 storage[0x40], +0x48 holder*, +0x50 type id.
// The exe copies the storage only through the holder's vtable (+0x08 destroy, +0x10 copy, +0x18 clone); the host's
// holder is a plain 0x40-byte copy.
unsafe extern "win64" fn ud_holder_dtor(_h: u64, _a: u64) -> u64 {
    0
}
unsafe extern "win64" fn ud_holder_destroy(_h: u64, _storage: u64) -> u64 {
    0
}
unsafe extern "win64" fn ud_holder_copy(_h: u64, dst: u64, src: u64) -> u64 {
    unsafe {
        let d = rd64(dst);
        let s = rd64(src);
        std::ptr::copy_nonoverlapping(s as *const u8, d as *mut u8, 0x40);
    }
    0
}
unsafe extern "win64" fn ud_holder_clone(h: u64, _span: u64) -> u64 {
    h
}
static UD_HOLDER: OnceLock<u64> = OnceLock::new();
fn ud_holder() -> u64 {
    *UD_HOLDER.get_or_init(|| unsafe {
        let vt = mem::heap_alloc(0x20, true) as u64;
        *(vt as *mut u64) = ud_holder_dtor as *const () as u64;
        *((vt + 8) as *mut u64) = ud_holder_destroy as *const () as u64;
        *((vt + 0x10) as *mut u64) = ud_holder_copy as *const () as u64;
        *((vt + 0x18) as *mut u64) = ud_holder_clone as *const () as u64;
        let h = mem::heap_alloc(0x10, true) as u64;
        *(h as *mut u64) = vt;
        h
    })
}
/// User data for the project request as the match cook gives it: the 0x48-byte game_project_query_user_data
/// starts with {game+0x38, options}; 0x2001D8670 checks the type id against 0x20012B4C0() and copies those 16 bytes.
fn project_user_data(game: u64, options: u64) -> u64 {
    unsafe {
        let w = mem::heap_alloc(0x58, true) as u64;
        *((w + 8) as *mut u64) = game + 0x38;
        *((w + 0x10) as *mut u64) = options;
        *((w + 0x48) as *mut u64) = ud_holder();
        let tid = call0(0x20012B4C0) as u32;
        *((w + 0x50) as *mut u32) = tid;
        hlog!("project user data {w:#x}: payload {{game+0x38 {:#x}, options {options:#x}}}, type id {tid:#x}", game + 0x38);
        w
    }
}

static ONE_RESULT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static ONE_FLAGS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// The entry's +0x1C0 slot: the raw (unmanaged) resource of class-3 files (managed resources come from +0x1C8).
static ONE_RAW: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// Callback for `query_one`: keeps the first resource (refcount +1 at resource+0x1B0, as the client does when it
/// stores one) and its entry flags.
unsafe extern "win64" fn one_query_cb(_buf: u64, result: u64) -> u64 {
    unsafe {
        let entry = result + 0x70;
        let res = rd64(entry + 0x1C8);
        let raw = rd64(entry + 0x1C0);
        if raw != 0 {
            let rc = &*((raw + 0x1D8) as *const std::sync::atomic::AtomicU32);
            rc.fetch_add(1, Ordering::SeqCst);
        }
        ONE_RAW.store(raw, Ordering::Relaxed);
        ONE_FLAGS.store(*((entry + 0x340) as *const u32) as u64, Ordering::Relaxed);
        if res != 0 {
            let rc = &*((res + 0x1B0) as *const std::sync::atomic::AtomicU32);
            rc.fetch_add(1, Ordering::SeqCst);
        }
        ONE_RESULT.store(res, Ordering::Relaxed);
    }
    0
}
/// The world part of the client's match-start resource callback 0x2001301A0. The client's match-start query also
/// loads HUD movies, scenes and projectile models; of the callback's writes only two are read by world code, and the
/// host makes those two the same way (no movies, scenes or HUD):
/// (a) the crosshair hit-detection accumulation time 0x200AA2284 (u32 ms, exe default 500; how long a player's hit
///     record lives, part of the history item) from static_game_parameters.options
///     `crosshair_hit_detection.accumulation_time` (seconds * 1000). HOST_HIT_ACCUM_MS=n replaces it (diagnosis),
///     HOST_NO_STATIC_PARAMS=1 skips (a).
/// (b) the bullet visual slots: per bullet type 0..3 a vector at game+0xF45E8 + type * 0x18 of 16-byte entries
///     {bullet, model}, count from 0x200A9CA70; a new bullet takes slot counter % count, so empty vectors fault. The
///     model only reaches the render scene, so the host's entries are {0, 0}. HOST_BULLET_SLOTS=n replaces the count
///     (diagnosis; 0 = leave the vectors empty).
pub fn match_start_world_part(game: u64) {
    unsafe {
        if std::env::var("HOST_NO_STATIC_PARAMS").is_err() {
            let (res, fl) = query_one("resources/gameplay/static_game_parameters.options", 0x1F, 0);
            if res == 0 {
                hlog!("static_game_parameters.options: query returned nothing (entry flags {fl:#x}); [0x200AA2284] stays {}", *(0x200AA2284 as *const u32));
            } else {
                let cfg = rd64(res + 0x208);
                let a = call2(0x2000CC0E0, cfg, 0x2009599B8); // 'crosshair_hit_detection'
                let b = call2(0x2000CC0E0, a, 0x2009599D0); // 'accumulation_time'
                let v = rd64(b);
                let x: f32 = if (v >> 44) & 0xF == 2 { f32::from_bits(v as u32) } else { (v as u32 as i32) as f32 };
                let ms = (x * *(0x2009E60E4 as *const f32)) as i32 as u32;
                let before = *(0x200AA2284 as *const u32);
                *(0x200AA2284 as *mut u32) = ms;
                hlog!("static_game_parameters.options {res:#x}: crosshair_hit_detection.accumulation_time {x} s (variant {v:#x}) -> [0x200AA2284] {before} -> {ms} ms");
            }
        }
        if let Some(v) = std::env::var("HOST_HIT_ACCUM_MS").ok().and_then(|v| v.parse::<u32>().ok()) {
            *(0x200AA2284 as *mut u32) = v;
            hlog!("HOST_HIT_ACCUM_MS: [0x200AA2284] := {v}");
        }
        let n: u64 = std::env::var("HOST_BULLET_SLOTS").ok().and_then(|v| v.parse().ok()).unwrap_or(*(0x200A9CA70 as *const u32) as u64);
        if n > 0 {
            let mut filled = 0;
            for t in 0..4u64 {
                let v = game + 0xF45E8 + t * 0x18;
                if rd64(v) == rd64(v + 8) {
                    let b = mem::heap_alloc((16 * n) as usize, true) as u64;
                    *(v as *mut u64) = b;
                    *((v + 8) as *mut u64) = b + 16 * n;
                    *((v + 0x10) as *mut u64) = b + 16 * n;
                    filled += 1;
                }
            }
            hlog!("bullet visual slots: {filled} of 4 type vectors at game+0xF45E8 given {n} entries {{0, no model}} ([0x200A9CA70] = {})", *(0x200A9CA70 as *const u32));
        }
    }
}
/// The game's start-up console config. The game runs "resources/startup.cfg" and "user_data/user.cfg" in a start-up
/// callback the host never runs. startup.cfg sets tuning cvars the world reads (e.g. cc_step_height 0.80, exe default
/// 0.6 at 0x200AA3498, used by the character controller's step probe), so the host runs it the same way, through the
/// exe's console executor 0x2000B1590; user.cfg is per-player and is not run. HOST_NO_STARTUP_CFG=1 skips it
/// (diagnosis).
fn apply_startup_cfg() {
    if std::env::var("HOST_NO_STARTUP_CFG").is_ok() {
        hlog!("startup.cfg: skipped (HOST_NO_STARTUP_CFG)");
        return;
    }
    let (managed, fl) = query_one("resources/startup.cfg", 3, 0);
    let res = ONE_RAW.load(Ordering::Relaxed);
    hlog!("startup.cfg: entry +0x1C8 {managed:#x}, +0x1C0 {res:#x}, flags {fl:#x}");
    if res == 0 {
        hlog!("startup.cfg: query returned nothing (entry flags {fl:#x})");
        return;
    }
    unsafe {
        let step = 0x200AA3498u64;
        let before = *(step as *const f32);
        let text = rd64(res + 0x1C8) + 0x30;
        let size = *((res + 0xF8) as *const u32);
        let head = std::slice::from_raw_parts(text as *const u8, (size as usize).min(48));
        hlog!("startup.cfg: resource {res:#x}, {size} bytes, starts {:?}", String::from_utf8_lossy(head));
        let desc = mem::heap_alloc(0x18, true) as u64;
        *(desc as *mut u64) = text;
        *((desc + 8) as *mut u64) = text;
        *((desc + 0x10) as *mut u32) = size;
        // third argument: the config kind (1 = startup.cfg, 2 = user.cfg), passed on to every line
        call3(0x2000B1590, desc, 1, 1);
        hlog!("startup.cfg executed (0x2000B1590): cc_step_height [0x200AA3498] {before} -> {}", *(step as *const f32));
    }
    // HOST_USER_CFG=<file>: also run a player's user.cfg after it (kind 2), as the client's start-up callback does.
    // Diagnosis only (which per-player cvars, if any, change the world).
    if let Ok(path) = std::env::var("HOST_USER_CFG") {
        match std::fs::read(&path) {
            Ok(b) => unsafe {
                let text = mem::heap_alloc(b.len() + 1, true) as u64;
                std::ptr::copy_nonoverlapping(b.as_ptr(), text as *mut u8, b.len());
                let desc = mem::heap_alloc(0x18, true) as u64;
                *(desc as *mut u64) = text;
                *((desc + 8) as *mut u64) = text;
                *((desc + 0x10) as *mut u32) = b.len() as u32;
                call3(0x2000B1590, desc, 1, 2);
                hlog!("user.cfg {path} executed (kind 2, {} bytes)", b.len());
            },
            Err(e) => hlog!("user.cfg {path}: {e}"),
        }
    }
}
/// One resource through the exe's query_resource_and_wait; returns (resource, entry flags).
pub fn query_one(path: &str, class: u64, user_data: u64) -> (u64, u64) {
    unsafe {
        let vt = mem::heap_alloc(16, true) as u64;
        *(vt as *mut u64) = functor_manager as *const () as u64;
        *((vt + 8) as *mut u64) = one_query_cb as *const () as u64;
        let functor = mem::heap_alloc(0x30, true) as u64;
        *(functor as *mut u64) = vt | 1;
        let name = crate::strings::leak_cstr(path);
        ONE_RESULT.store(0, Ordering::Relaxed);
        let f: extern "win64" fn(u64, u64, u64, u64, u64, u64) -> u64 = std::mem::transmute(0x2000A9EF0u64);
        f(name, class, functor, rd64(0x20F43E400), user_data, 0);
    }
    (ONE_RESULT.load(Ordering::Relaxed), ONE_FLAGS.load(Ordering::Relaxed))
}
/// Step 3 experiment: the project resource (class 0x40, name as the match cook passes it, 0x2002E6710).
fn step3(o: &Opts) -> Result<(), String> {
    // The world thread's FPU state. Every engine thread runs the exe's FPU setup 0x2000B0930 (denormals flushed,
    // flush-to-zero; skipped when the cvar 0x200AAF188 is 1). The host's world thread is a Rust thread (Linux default
    // MXCSR 0x1F80), so the same setup runs here; HOST_MXCSR=<hex> overrides the result (float drift diagnosis).
    unsafe {
        let before = std::arch::x86_64::_mm_getcsr();
        if std::env::var("HOST_NO_FPU_SETUP").is_err() {
            call0(0x2000B0930);
        }
        let mid = std::arch::x86_64::_mm_getcsr();
        // x87 control word: Windows starts x64 threads with 0x27F (53-bit precision), Linux with 0x37F (64-bit).
        // HOST_X87_CW=<hex> sets it on the world thread (drift diagnosis); the value before/after is logged.
        let mut cw: u16 = 0;
        std::arch::asm!("fnstcw [{}]", in(reg) &mut cw);
        let cw_before = cw;
        if let Some(v) = std::env::var("HOST_X87_CW").ok().and_then(|v| u16::from_str_radix(v.trim_start_matches("0x"), 16).ok()) {
            std::arch::asm!("fldcw [{}]", in(reg) &v);
        }
        std::arch::asm!("fnstcw [{}]", in(reg) &mut cw);
        hlog!("world thread x87 control word {cw_before:#06x} -> {cw:#06x}");
        if let Some(v) = std::env::var("HOST_MXCSR").ok().and_then(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok()) {
            std::arch::x86_64::_mm_setcsr(v);
        }
        hlog!(
            "world thread FPU: MXCSR {before:#06x} -> 0x2000B0930 (cvar [0x200AAF188] = {}) -> {mid:#06x} -> now {:#06x}",
            *(0x200AAF188 as *const u32),
            std::arch::x86_64::_mm_getcsr()
        );
    }
    // The game object (which registers the game cooks, among them 0x40 project and 0x68 pvp_match_resource) comes
    // from the game factory 0x200107CA0(engine, render world, sound world, network world), as in the client.
    let ew = unsafe { rd64(0x200AB16B0) };
    if std::env::args().any(|a| a == "--render-world") {
        // the client's two render-world objects (ctor 0x2005FD550 on their static storage) without any device;
        // engine_world+0xDD8 points at the first
        unsafe {
            call2(0x2005FD550, 0x2034C3D90, rd64(0x20F43E400));
            let second = if rd8(0x20F43E374) != 0 { ew + 0xA10 } else { 0 };
            call2(0x2005FD550, 0x2034C3E60, second);
            *((ew + 0xDD8) as *mut u64) = 0x2034C3D90;
        }
        hlog!("render world objects built (0x2034C3D90, 0x2034C3E60), no device");
        if std::env::args().any(|a| a == "--null-render") {
            null_render_init(ew)?;
        }
    }
    let (render, sound, net) = unsafe { (rd64(ew + 0xDD8), rd64(ew + 0xDF8), rd64(ew + 0xDF0)) };
    hlog!("=== step 3: game object via 0x200107CA0(engine {:#x}, render world {render:#x}, sound world {sound:#x}, network world {net:#x})", ew + 8);
    // The render window handle at engine_world+0xE20 and +0xE30: engine init stores it from a window function the host
    // skips, and the game's initialise waits for it, so the host stores its own handle.
    unsafe {
        *((ew + 0xE20) as *mut u64) = crate::other::HOST_HWND;
        *((ew + 0xE30) as *mut u64) = crate::other::HOST_HWND;
    }
    hlog!("engine_world+0xE20/+0xE30 (render window handle) := host handle {:#x} (no real window)", crate::other::HOST_HWND);
    // The game ctor's own start-up query has the game's UI/window/input initialise 0x200111290 as its completion
    // callback, delivered to the creating thread's dispatch. The host
    // creates the game on a separate thread that then parks without ever pumping, so that callback never runs.
    let (tx, rx) = std::sync::mpsc::channel();
    let world_tid = teb::gettid();
    let on_world = std::env::args().any(|a| a == "--game-on-world");
    if !on_world { mem::spawn_low(
        32 << 20,
        Box::new(move || {
            teb::setup_current_thread();
            teb::run_tls_callbacks(2);
            hlog!("game-creation thread {} starts the factory (reports thread id {world_tid})", teb::gettid());
            crate::k32::REPORTED_TID.with(|t| t.set(world_tid));
            let g = unsafe { call4(0x200107CA0, ew + 8, render, sound, net) };
            crate::k32::REPORTED_TID.with(|t| t.set(0));
            let _ = tx.send(g);
            loop {
                std::thread::sleep(std::time::Duration::from_secs(3600));
            }
        }),
    ); }
    let g = if on_world {
        hlog!("creating the game on the world thread (its start-up callback 0x200111290 will run in the pump)");
        unsafe { call4(0x200107CA0, ew + 8, render, sound, net) }
    } else {
        rx.recv().map_err(|e| format!("game thread: {e}"))?
    };
    hlog!("game factory returned {g:#x}");
    unsafe {
        // cook objects record their thread at +0x18 (GetCurrentThreadId at registration)
        hlog!(
            "cook thread ids: 0x68 pvp_match (0x20F442720) -> {}, 0x40 project (0x2126602F0, not yet registered) -> {}; world thread {}",
            *((0x20F442738u64) as *const u32),
            *((0x212660308u64) as *const u32),
            teb::gettid()
        );
    }
    GAME.store(g, Ordering::Relaxed);
    unsafe {
        hlog!(
            "game fields: +0x10FCF8 engine {:#x}, +0x10FD00 render {:#x}, +0x10FD08 sound {:#x}, +0x10FD10 network {:#x}, +0x10FD28 {:#x}, +0x288 {:#x}",
            rd64(g + 0x10FCF8),
            rd64(g + 0x10FD00),
            rd64(g + 0x10FD08),
            rd64(g + 0x10FD10),
            rd64(g + 0x10FD28),
            rd64(g + 0x288)
        );
    }
    unsafe { *((ew + 0xE08) as *mut u64) = g }; // the engine's game pointer, as the client sets it
    // The level cooks (class 0x40 project and others) are registered by 0x200118090(game), which the client reaches
    // from its initial-load callbacks; those also set up UI/render, so the host calls only this one.
    unsafe { call1(0x200118090, g) };
    hlog!("cook registration 0x200118090(game) returned; class-0x40 cook object 0x2126602F0 vtable {:#x}", unsafe { rd64(0x2126602F0) });
    install_null_cooks()?;
    // The level's objects look items up in the game's items dictionary (game+0x10FE20, "items_dictionary", class
    // 0x42), which the client loads in its start-up chain. The host loads just that resource the same way.
    if !std::env::args().any(|a| a == "--game-on-world") {
        let (dict, fl) = query_one("items_dictionary", 0x42, 0);
        hlog!("items_dictionary (class 0x42) -> {dict:#x} (entry flags {fl:#x})");
        if dict == 0 {
            return Err("items_dictionary did not load".into());
        }
        unsafe { *((GAME.load(Ordering::Relaxed) + 0x10FE20) as *mut u64) = dict };
    }
    apply_startup_cfg();
    hlog!("=== step 3: level project '{}' (class 0x40) through query_resource_and_wait", o.level);
    arm_debug("project");
    // Bullet's global aligned-alloc hooks (0x200AA36C0 / 0x200AA36C8): in the client the lobby's physics world points
    // them at the physics allocator long before any level loads. The host has no lobby, so it sets them itself;
    // otherwise level objects allocated by the CRT malloc are freed into the physics arena and abort.
    unsafe {
        *(0x200AA36C0 as *mut u64) = 0x2005E5280;
        *(0x200AA36C8 as *mut u64) = 0x2005E52A0;
    }
    hlog!("bullet alloc hooks: [0x200AA36C0] = 0x2005E5280, [0x200AA36C8] = 0x2005E52A0 (as after the client's lobby physics world init)");
    // HOST_PHYS_PAD=<bytes>: allocate that much from the physics allocator (global 0x20F4404E8) before the level loads, shifting every later physics address (drift diagnosis:
    // does the result depend on addresses?)
    if let Some(n) = std::env::var("HOST_PHYS_PAD").ok().and_then(|v| v.parse::<u64>().ok()) {
        unsafe {
            let a = rd64(0x20F4404E8);
            let f: extern "win64" fn(u64, u64, u64) -> u64 = std::mem::transmute(rd64(rd64(a) + 0x20));
            let p = f(a, n, 16);
            hlog!("physics pad: {n} bytes from the physics allocator {a:#x} at {p:#x}");
        }
    }
    // The game's localization holder game+0x10FC28 (loaded by 0x20013DFA0 in the game's initialise): PvE script HUD
    // messages read it during the world step and fault when it is empty. Issued here so it loads with the level project.
    unsafe {
        let game = GAME.load(Ordering::Relaxed);
        if game != 0 && rd64(game + 0x10FC28) == 0 {
            match crate::k32::call_fault_guarded(0x20013DFA0, game + 0x10FC28) {
                Ok(_) => hlog!("localization holder: 0x20013DFA0(game+0x10FC28) returned; [holder] = {:#x}", rd64(game + 0x10FC28)),
                Err(c) => hlog!("localization holder: 0x20013DFA0 left after a fault (code {c:#x})"),
            }
        }
    }
    if std::env::args().any(|a| a == "--trace3") {
        crate::thunks::TRACE.store(true, Ordering::Relaxed);
    }
    unsafe {
        let vt = mem::heap_alloc(16, true) as u64;
        *(vt as *mut u64) = functor_manager as *const () as u64;
        *((vt + 8) as *mut u64) = project_query_cb as *const () as u64;
        let functor = mem::heap_alloc(0x30, true) as u64;
        *(functor as *mut u64) = vt | 1;
        let name = crate::strings::leak_cstr(&o.level);
        let alloc = rd64(0x20F43E400);
        let f: extern "win64" fn(u64, u64, u64, u64, u64, u64) -> u64 = std::mem::transmute(0x2000A9EF0u64);
        let ud = project_user_data(GAME.load(Ordering::Relaxed), 0);
        f(name, 0x40, functor, alloc, ud, 0);
    }
    render_channel_report("after project query");
    mem_snapshot("after project query (step 3)");
    mem_breakdown("after project query (step 3)");
    let r = PROJ_RESULT.lock().unwrap().clone();
    if !r.iter().any(|l| l.contains("static geometry count")) {
        return Err("project did not load".into());
    }
    let get = |k: &str| {
        let a: Vec<String> = std::env::args().collect();
        a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned()
    };
    if let Some(z) = crate::daemon::ZygoteOpts::from_args() {
        // daemon zygote: pre-warm, then fork one child per handed-over match (the children never return here)
        arm_debug("match");
        crate::world::MatchWorld::prepare_process()?;
        if let Some(p) = get("--prewarm") {
            prewarm(&p)?;
        }
        // the level load queued its render commands (about 7000 on pve_01, of the pool's 8192): retire them once here,
        // so every child starts with an empty render pool instead of retiring (and copying) them itself
        let (n, marked) = crate::world::retire_render_commands();
        hlog!("zygote: {n} render commands of the level load retired ({marked} with keep marks); render pool in use {}", unsafe { *((0x2034C3F40u64 + 0x40) as *const u32) });
        return crate::daemon::zygote_loop(&z);
    }
    if let Some(mm) = get("--match") {
        let (map, mode) = mm.split_once(',').ok_or("--match MAP,MODE")?;
        step3_match(GAME.load(Ordering::Relaxed), map.parse().map_err(|_| "map")?, mode.parse().map_err(|_| "mode")?)?;
    }
    Ok(())
}

/// RTTI class name of an object's vtable (MSVC x64: vtable[-1] = complete object locator, COL+0x0C = type
/// descriptor RVA, name at TD+0x10).
pub fn rtti_name(vt: u64) -> String {
    unsafe {
        if !(0x200000000..0x210000000).contains(&vt) {
            return "?".into();
        }
        let col = rd64(vt - 8);
        if !(0x200000000..0x210000000).contains(&col) {
            return "?".into();
        }
        let td = 0x200000000 + *((col + 0xC) as *const u32) as u64;
        std::ffi::CStr::from_ptr((td + 0x10) as *const std::ffi::c_char).to_string_lossy().into_owned()
    }
}

/// The resource manager's cook table (register_cook 0x2000B7200: buffer_vector at 0x200AAF1A0, 0x20A slots,
/// cook stored at begin[class = cook+0xC]).
pub fn log_cook_table(label: &str) {
    unsafe {
        let begin = rd64(0x200AAF1A0);
        let mut n = 0;
        let mut line = String::new();
        for c in 0..0x20Au64 {
            let cook = rd64(begin + 8 * c);
            if cook != 0 {
                n += 1;
                // the cook's thread id +0x18 (-4: resolved lazily)
                line.push_str(&format!(" {c:#x}={}[t{}]", rtti_name(rd64(cook)).trim_start_matches(".?AV").trim_end_matches("@@"), *((cook + 0x18) as *const i32)));
            }
        }
        hlog!("cook table [{label}]: {n} classes registered:{line}");
    }
}

/// The model-cook subset of the client's render init over the null DXGI/D3D11 device (`d3d.rs`), in client order and
/// with the client's arguments: after the two render worlds (done by the caller) the render engine ctor 0x200470B90,
/// the render scene init 0x2005FD980, the render cook registration 0x2004A3940, the device 0x200470EA0 and
/// allocate_geometry_pool 0x200454010. Not run: window cook 0x77, render stages, renderer 0x78, the :render thread.
fn null_render_init(ew: u64) -> Result<(), String> {
    unsafe {
        let res = rd64(ew + 0xDE8);
        let (a8, a9) = (rd64(0x20C08D310), rd64(0x20C08D318));
        let flag = rd8(0x20F43E374) as u64;
        hlog!("null render: engine_world+0xDE8 = {res:#x}, [0x20C08D310] = {a8:#x}, [0x20C08D318] = {a9:#x}, byte [0x20F43E374] = {flag}");
        // the logic thread's render allocator: 0x2005FD4E0(engine_world+0x9C8) stores it at 0x20F4401F0, where the
        // texture cook takes it from (the client posts this call to the game module; null there faults)
        hlog!("null render: [0x20F4401F0] = {:#x}, engine_world+0x9C8 vtable {:#x} {}, 0x2000F1DF0() = {}", rd64(0x20F4401F0), rd64(ew + 0x9C8), rtti_name(rd64(ew + 0x9C8)), call0(0x2000F1DF0) as u8);
        call1(0x2005FD4E0, ew + 0x9C8);
        hlog!("null render: 0x2005FD4E0(engine_world+0x9C8) -> [0x20F4401F0] = {:#x}", rd64(0x20F4401F0));
        let functor = mem::heap_alloc(0x40, true) as u64;
        *(functor as *mut u64) = 0x200923721;
        *((functor + 8) as *mut u64) = 0x200088F70;
        *((functor + 0x10) as *mut u64) = rd64(0x200AA3910);
        let ctor: extern "win64" fn(u64, u64, u64, u64, u64, u64, u64) -> u64 = std::mem::transmute(0x200470B90u64);
        ctor(0x200AF6A48, ew + 0xDE8, a8, a9, 0x3840_0000, flag, functor);
        hlog!("null render: render engine ctor 0x200470B90 returned");
        log_cook_table("after the render engine ctor");
        (*(0x200AF6AA8 as *const std::sync::atomic::AtomicU32)).swap(1, Ordering::SeqCst);
        let r8 = rd64(0x200AF6AA0);
        *(0x2034C3F30 as *mut u64) = r8;
        *(0x2034C3F38 as *mut u64) = r8;
        call3(0x2005FD980, 0x2034C3F40, 0x2034C3D90, r8);
        *(0x204114158 as *mut u32) = 1;
        *(0x20411415C as *mut u32) = 0;
        *(0x204114160 as *mut u16) = 1;
        *((ew + 0xDD8) as *mut u64) = 0x2034C3D90;
        hlog!("null render: scene init 0x2005FD980(0x2034C3F40, 0x2034C3D90, {r8:#x}) returned; scene allocator [0x2034C3F40+0xC50138+0x10] = {:#x}", rd64(0x2034C3F40 + 0xC50138 + 0x10));
        call2(0x2004A3940, 0, flag);
        hlog!("null render: cook registration 0x2004A3940 returned");
        // the render channel: the client posts the one_way_render_channel init 0x2000F39D0 on the render world to
        // its render thread; without it adding a model to the scene faults. Here it runs on the world thread and
        // nothing consumes the channel (counted by render_channel_report).
        call1(0x2000F39D0, 0x2034C3D90);
        hlog!("null render: render channel 0x2000F39D0(0x2034C3D90): +8 {:#x}, +0x48 {:#x}, +0x58 thread {}", rd64(0x2034C3D90 + 8), rd64(0x2034C3D90 + 0x48), *((0x2034C3D90u64 + 0x58) as *const u32));
        let eng = rd64(0x2034C3D90 + 0x1A8);
        hlog!("null render: [render_world+0x1A8] = {eng:#x} (render engine object 0x200AF6A48)");
        call1(0x200470EA0, if eng != 0 { eng } else { 0x200AF6A48 });
        hlog!("null render: device 0x200470EA0 returned; device [0x2034C3A08] = {:#x}, context [0x2034C3A18] = {:#x}, level [0x2034C3A20] = {:#x}", rd64(0x2034C3A08), rd64(0x2034C3A18), *(0x2034C3A20 as *const u32));
        // the client's next step after the device: the render device helper allocator, a 16 MB static buffer the
        // texture cook and texture utilities allocate from
        let helper = call0(0x200470B00);
        let init: extern "win64" fn(u64, u64, u64, u64) -> u64 = std::mem::transmute(rd64(rd64(helper) + 8));
        init(helper, 0x212D9C010, 0x100_0000, 0x2009ADDE8);
        hlog!("null render: render_device_helper_allocator {helper:#x} set up over 0x212D9C010 (16 MB); [render_world+0x1A0] = {:#x}", rd64(0x2034C3D90 + 0x1A0));
        let a = (call1(0x20008EB70, 0x200AF2D20) as u32) << 20;
        let b = (call1(0x20008EB70, 0x200AF2D70) as u32) << 20;
        call2(0x200454010, a as u64, b as u64);
        hlog!("null render: allocate_geometry_pool 0x200454010({:#x}, {:#x}) returned; {} null D3D children", a, b, crate::d3d::children());
        mem_snapshot("after the null render init");
        mem_breakdown("after the null render init");
        log_cook_table("after the null render init");
    }
    Ok(())
}

/// --null-cooks 0x3d,...|empty: classes answered with empty resources (render / sound / particle side only).
fn install_null_cooks() -> Result<(), String> {
    unsafe {
        // --model-cooks: the exe's own model-instance cooks, registered as the render init does: static object
        // {+0 vtable, +8 0, +0xC class, +0x14 -1, +0x18 -4 (thread resolved lazily), +0x1C 8, +0x20 0}, then
        // register_cook 0x2000B7200. Their instances hold the render model at +0x208 and the skeleton (class 0x2C) at
        // +0x210.
        if std::env::args().any(|a| a == "--model-cooks") {
            let mut list = vec![
                (0x213D9C950u64, 0x2009B69F8u64, 0xEu64), // static_model_instance_cook
                (0x213D9CA50, 0x2009B7FC8, 0xF),          // skeleton_model_instance_cook
                (0x213D9CAE8, 0x2009B8270, 0x10),         // skeleton_combined_model_instance_cook
            ];
            if std::env::args().any(|a| a == "--render-model-cooks") {
                // experiment: the render-side model cooks too
                list.extend([
                    (0x213D9CA80, 0x2009B81F8, 0x14), // skeleton_render_model_instance_cook
                    (0x213D9CB18, 0x2009B87C8, 0x15), // skeleton_combined_render_model_instance_cook
                    (0x213D9CB48, 0x2009B8AF8, 0x16), // skeleton_combined_model_cook
                ]);
            }
            for (obj, vt, class) in list {
                *(obj as *mut u64) = vt;
                *((obj + 8) as *mut u32) = 0;
                ((obj + 0xC) as *mut u64).write_unaligned(class);
                *((obj + 0x14) as *mut u32) = 0xFFFF_FFFF;
                *((obj + 0x18) as *mut u32) = 0xFFFF_FFFC;
                *((obj + 0x1C) as *mut u32) = 8;
                *((obj + 0x20) as *mut u64) = 0;
                call1(0x2000B7200, obj);
                hlog!("model cook class {class:#x} registered: {} at {obj:#x}", rtti_name(vt));
            }
        }
        // the classes listed after --null-cooks (or every class without a cook: "empty")
        let args: Vec<String> = std::env::args().collect();
        if let Some(list) = args.iter().position(|a| a == "--null-cooks").and_then(|i| args.get(i + 1)) {
            // "render": the drawing/sound-only classes a world host can answer empty:
            // shaders 0xA, effects 0xB, geometry 0x96 / shadow proxy 0x97, render models 0x11 / 0x12, render model
            // instances 0x13 / 0x14 / 0x18, grass 0x19 / 0x79, particles 0x3A / 0x3B / 0x3D, texture options 0x34,
            // raw texture data 0xC2, sounds 0x27 / 0x28 / 0x29 / 0x100 / 0x101 / 0x102. Kept on purpose: 0x15
            // skeleton_combined_render_model_instance and 0x16 skeleton_combined_model (the players' bind poses come
            // from them) and 0xE static_model_instance (the level load fails without it).
            const RENDER: &str = "a,b,96,97,11,12,13,14,18,19,79,3a,3b,3d,34,c2,27,28,29,100,101,102";
            let list = list.replace("render", RENDER);
            for c in list.split(',') {
                if let Some(range) = c.strip_prefix("empty") {
                    // every class without a registered cook ("empty"), or in a range ("empty1-3f")
                    let (lo, hi) = match range.split_once('-') {
                        Some((a, b)) => (u64::from_str_radix(a, 16).unwrap_or(0), u64::from_str_radix(b, 16).unwrap_or(0x209) + 1),
                        None => (0, 0x20A),
                    };
                    let begin = rd64(0x200AAF1A0);
                    let mut n = 0;
                    for k in lo..hi.min(0x20A) {
                        if rd64(begin + 8 * k) == 0 {
                            crate::nullcook::install(k as u32);
                            n += 1;
                        }
                    }
                    hlog!("null cooks installed for all {n} classes without a cook");
                    continue;
                }
                let c = u32::from_str_radix(c.trim_start_matches("0x"), 16).map_err(|_| format!("bad class {c}"))?;
                crate::nullcook::install(c);
            }
        }
    }
    Ok(())
}

/// A u32 in decimal or 0x hex.
pub fn parse_u32(s: &str) -> Option<u32> {
    match s.strip_prefix("0x") {
        Some(h) => u32::from_str_radix(h, 16).ok(),
        None => s.parse().ok(),
    }
}

/// Replace the match id of an options blob: header `u16 send_period, u16 max_delay, u8 tick_ms, u8 n_players,
/// u32 match_id` (local_match options.rs online header), read by 0x2001250E0 into opts+0x1074.
pub fn set_options_match_id(blob: &mut [u8], id: u32) -> Result<(), String> {
    let b = blob.get_mut(6..10).ok_or("options blob shorter than its header")?;
    b.copy_from_slice(&id.to_le_bytes());
    Ok(())
}

/// The match options blob exactly as our server sends it in DD (`local_match::online_options`, one human).
fn match_options_blob(map: u32, mode: u32, match_id: u32) -> Result<Vec<u8>, String> {
    let gd = crate::run::game_tables()?;
    let pve = std::env::var("HOST_PVE").ok().and_then(|v| v.split_once(':').and_then(|(d, m)| Some((d.parse::<u8>().ok()?, m.parse::<u8>().ok()?))));
    // the row of (map, mode): variation 0, else the first variation that has one (map 5 slaughter has no variation 0)
    let (row, variation) = if pve.is_some() {
        (None, 0)
    } else {
        let v = (0..16u32).find(|&v| gd.matches.resolve_map(map, mode, v).is_some_and(|r| v == 0 || r.variation == v)).ok_or("no mode row")?;
        (gd.matches.resolve_map(map, mode, v), v)
    };
    if let Some(row) = row {
        hlog!("match row map {map} mode {mode} variation {variation}: project '{}', map name '{}'", row.project, row.map_name);
    }
    let tick = local_match::options::TICK_MS as u8;
    let p = local_match::OnlineParams {
        map,
        mode,
        weather: variation,
        match_id,
        tick_ms: tick,
        send_period_ms: local_match::options::SEND_PERIOD_MS,
        match_type: local_match::options::MATCH_TYPE_PLAIN,
        max_delay_ms: local_match::options::max_delay_ms(tick),
    };
    // HOST_PLAYERS=n: n default test humans, teams alternating (a full roster for the creation measurements); HOST_PVE=d:m:
    // the PvE options of difficulty d (0..2), maze m (1..2), level 1, with that many humans (the mission's bots added)
    let n: usize = std::env::var("HOST_PLAYERS").ok().and_then(|v| v.parse().ok()).unwrap_or(1).clamp(1, 20);
    let players: Vec<local_match::OnlinePlayer> =
        (0..n).map(|i| local_match::OnlinePlayer { team: (i % 2) as u8, human: local_match::HumanLoadout::default_loadout(), hero: None, pve_budget: (0, 0) }).collect();
    let blob = match pve {
        Some((d, m)) => local_match::pve_options(&gd, &p, &players, d, m, 1).map_err(|e| format!("PvE options: {e}"))?,
        None => local_match::online_options(&gd, &p, &players).map_err(|e| format!("options: {e}"))?,
    };
    hlog!("built options: {} bytes, {n} human(s){}", blob.len(), std::env::var("HOST_PVE").map(|v| format!(", PvE {v}")).unwrap_or_default());
    let Some(row) = row.filter(|_| std::env::args().any(|a| a == "--no-players")) else {
        return Ok(blob);
    };
    // --no-players: the same blob with an empty roster: n_players (byte 5) = 0 and the player record removed. Layout
    // (options.rs online_blob): 16-byte header, u8 length + project, u8 length + map name (wire.rs str_raw), the
    // player records, then 100 bytes of tables (20 + 40 zero bytes, 20 x {hero place, nomination}).
    let pos = 16 + 1 + row.project.len() + 1 + row.map_name.len();
    if blob.get(17..17 + row.project.len()) != Some(row.project.as_bytes())
        || blob.get(pos - row.map_name.len()..pos) != Some(row.map_name.as_bytes())
        || blob.len() < pos + 100
    {
        return Err("options layout not as expected".into());
    }
    let mut out = blob[..pos].to_vec();
    out[5] = 0;
    out.extend_from_slice(&blob[blob.len() - 100..]);
    hlog!("options with an empty roster: {} bytes (player record of {} bytes removed)", out.len(), blob.len() - out.len());
    Ok(out)
}

/// Step 3 by the client's own match chain, through [`crate::world::MatchWorld`]: the options
/// blob (`--options-file`, a recording's DD body, or a built one for `--match MAP,MODE`) and the first world
/// (`--de-file`; without it the first history item is made by 0x2002F04F0, the bind-pose check), then optionally the
/// replay of a recording (`--replay`). `--fork-match N` (zygote test): the parent pre-warms (`HOST_PREWARM_LIST`) and
/// forks N children, each of which creates the match and replays it.
fn step3_match(_game: u64, map: u32, mode: u32) -> Result<(), String> {
    let match_id = 1795386319u32; // a fixed default match id (the world's seed); --match-id replaces it
    let a: Vec<String> = std::env::args().collect();
    let arg = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    let mut blob = match arg("--options-file") {
        Some(p) => std::fs::read(&p).map_err(|e| format!("{p}: {e}"))?,
        None => match_options_blob(map, mode, match_id)?,
    };
    // --match-id N: the options' match id (header bytes 6..10 = opts+0x1074, the world's seed) replaced
    if let Some(id) = arg("--match-id").and_then(|x| parse_u32(&x)) {
        set_options_match_id(&mut blob, id)?;
        hlog!("options match id set to {id:#x}");
    }
    arm_debug("match");
    log_cook_table("before pvp_match_resource");
    // host bots (bots.rs): HOST_BOTS=pve|pvp, read before the match creation (PvE writes the user data's +0x38)
    crate::bots::set_plan(crate::bots::plan_from_env(false));
    // offline test: HOST_BOTS_SYNTH=FILE writes a synthetic PvP recording with bots (bots.rs) and stops
    if let Ok(out) = std::env::var("HOST_BOTS_SYNTH") {
        return crate::bots::synth_pvp(map, mode, &out);
    }
    if let Some(n) = arg("--fork-match").and_then(|x| x.parse::<u32>().ok()) {
        // as the daemon's zygote: the process objects, the pre-warm list, the level load's render commands retired
        crate::world::MatchWorld::prepare_process()?;
        if let Ok(path) = std::env::var("HOST_PREWARM_LIST") {
            prewarm(&path)?;
        }
        let (r, _) = crate::world::retire_render_commands();
        hlog!("parent: {r} render commands of the level load retired; render pool in use {}", crate::world::render_pool_in_use());
        if fork_children(n) {
            return Ok(());
        }
    }
    let de = match arg("--de-file") {
        Some(p) => Some(std::fs::read(&p).map_err(|e| format!("{p}: {e}"))?),
        None => None,
    };
    if std::env::var("HOST_DUMP_OPTIONS").is_ok() {
        let _ = std::fs::write(format!("{}/options.bin", out_dir()), &blob);
    }
    let t_create = std::time::Instant::now();
    // --own-start (no --de-file): the host builds the starting world itself (MatchWorld::create_own) and writes it to
    // <out>/start_item.bin
    let mut mw = if de.is_none() && a.iter().any(|x| x == "--own-start") {
        let (mw, item) = crate::world::MatchWorld::create_own(&blob, a.iter().any(|x| x == "--own-first-step"))?;
        let _ = std::fs::write(format!("{}/start_item.bin", out_dir()), &item);
        hlog!("own start item: {} bytes -> {}/start_item.bin", item.len(), out_dir());
        mw
    } else {
        crate::world::MatchWorld::create(&blob, de.as_deref())?
    };
    hlog!("MATCH CREATED in {} ms; render pool in use {} of 8192", t_create.elapsed().as_millis(), crate::world::render_pool_in_use());
    let k = FORK_CHILD.load(Ordering::Relaxed);
    if k != 0 {
        mem_snapshot(&format!("child {k}: match created"));
        mem_breakdown(&format!("child {k}: match created"));
    }
    mem_snapshot("after match resource (step 3)");
    if let Some(rec) = arg("--replay") {
        let mut out = out_dir();
        if k != 0 {
            out = format!("{out}/child{k}");
            let _ = std::fs::create_dir_all(&out);
        }
        let mut bots = crate::bots::HostBots::start(mw.core, &out)?;
        if bots.is_some() {
            mem_snapshot(&format!("child {k}: bots started"));
        }
        replay(&mut mw, &rec, &out, bots.as_mut())?;
    }
    if k != 0 {
        mem_snapshot(&format!("child {k}: end"));
        hlog!("[child {k}] done");
        crate::log::flush();
        unsafe { libc::_exit(0) };
    }
    mem_snapshot("after level load (step 3)");
    mem_breakdown("after level load (step 3)");
    Ok(())
}

/// The pre-warm list (lines "class<TAB>name", made from a `HOST_QUERY_LOG` queries.tsv by `world-host prewarm-list`):
/// the zygote queries these shareable sub-resources of a match and keeps one reference each, so a child's match
/// creation finds them loaded. Returns (kept, empty).
pub fn prewarm(path: &str) -> Result<(u32, u32), String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
    let t0 = std::time::Instant::now();
    let (mut ok, mut none) = (0, 0);
    for l in text.lines() {
        let Some((c, n)) = l.split_once('\t') else { continue };
        let Ok(c) = u64::from_str_radix(c.trim_start_matches("0x"), 16) else { continue };
        let (r, _) = query_one(n, c, 0);
        if r != 0 || ONE_RAW.load(Ordering::Relaxed) != 0 {
            ok += 1;
        } else {
            none += 1;
        }
    }
    hlog!("prewarm list {path}: {ok} resources kept, {none} empty, in {} ms", t0.elapsed().as_millis());
    mem_snapshot("after the prewarm list");
    Ok((ok, none))
}

/// The game tables for the one-shot runs: `HOST_DATA_DIR` (the folder `srp setup` writes them to).
pub fn game_tables() -> Result<game_data::GameData, String> {
    let d = std::env::var_os("HOST_DATA_DIR").ok_or("set HOST_DATA_DIR to the folder of the game tables (`srp setup` writes them)")?;
    game_data::GameData::from_dir(std::path::Path::new(&d)).map_err(|e| format!("game data: {e}"))
}

/// The output directory when `--out` is not given (relative to the working directory).
const DEFAULT_OUT: &str = "world-host-out";

/// The host's output directory (`--out`).
pub fn out_dir() -> String {
    OUT.get().map(|p| p.display().to_string()).unwrap_or_else(|| DEFAULT_OUT.into())
}

/// One tick's E0 data as the client's E0 producer computes it: the exe's writer + hasher 0x2002FC5A0 on the newest
/// history item with time <= t and the presence node of t. Afterwards item+0x106244.. holds hash[20], mask[3] at
/// +0x106258, g0 +0x10625B, g1 +0x10625C, g2 +0x10625D.
/// Returns (item time, the 26 bytes item+0x106244.., the writer's stream). One reused 0x44800-byte stream buffer.
pub fn tick_e0(core: u64, t: u32, full: u8) -> Option<(u32, [u8; 26], Vec<u8>)> {
    static BUF: AtomicU64 = AtomicU64::new(0);
    unsafe {
        let mut item = rd64(core + 0x8008);
        while item != 0 && *((item + 0x106238) as *const u32) > t {
            item = rd64(item + 0x106228);
        }
        if item == 0 {
            return None;
        }
        let mut pres = rd64(core + 0x16C4F0);
        while pres != 0 && *((pres + 0x18) as *const u32) > t {
            pres = rd64(pres + 8);
        }
        let it = *((item + 0x106238) as *const u32);
        let mut out = Vec::new();
        if rd8(item + 0x10625E) == 0 || full != 0 {
            let mut b = BUF.load(Ordering::Relaxed);
            if b == 0 {
                b = mem::heap_alloc(0x44800 + 0x30, true) as u64;
                BUF.store(b, Ordering::Relaxed);
            }
            let (wr, inner, data) = (b, b + 0x10, b + 0x30);
            std::ptr::write_bytes(b as *mut u8, 0, 0x30);
            *(wr as *mut u8) = 0xFF;
            *((wr + 8) as *mut u64) = inner;
            *((inner + 8) as *mut u64) = data;
            *((inner + 0x10) as *mut u32) = 0x44800;
            let (p10, p14) = if pres != 0 { (pres + 0x10, pres + 0x14) } else { (0, 0) };
            let f: extern "win64" fn(u64, u64, u64, u64, u64, u8) -> u64 = std::mem::transmute(0x2002FC5A0u64);
            f(item, wr, p10, p14, p14, full);
            let n = (*((inner + 0x14) as *const u32) as usize).min(0x44800);
            out = std::slice::from_raw_parts(data as *const u8, n).to_vec();
        }
        let mut h = [0u8; 26];
        h.copy_from_slice(std::slice::from_raw_parts((item + 0x106244) as *const u8, 26));
        Some((it, h, out))
    }
}

/// A resync DE body: the item of T (full = 1) followed by the per-player animation-history tail the client's
/// deserialize 0x2002F1650 reads after the item.
#[derive(Clone, Debug, Default)]
pub struct DeBody {
    pub t: u32,
    /// bytes of the item part (writer 0x2002FC5A0, full = 1)
    pub item_len: usize,
    /// the presence mask at stream +0x11 (`*(pres_T+0x14)`), which the loader's tail loop walks lowest bit first
    pub mask: u32,
    /// first tail tick: `T < ACC ? 0 : floor_tick(T - ACC - tick) + tick` (ACC = options +0x107A, tick = options +0x1083)
    pub first_tail_tick: u32,
    pub tick: u32,
    pub blocks: u32,
    /// tail ticks whose history item was missing (one zero byte each: "no animation")
    pub missing: u32,
    /// tail bytes per present player (slot, bytes)
    pub per_player: Vec<(u8, usize)>,
    pub bytes: Vec<u8>,
}

/// Decoded DE size the client accepts (its DE buffer, 0x44800).
pub const DE_DECODED_MAX: usize = 0x44800;
/// The relay's DE payload limit (`DE_MAX_PAYLOAD`); the relay writes stored LZMA2 chunks (3 bytes per 64 KiB + 1).
pub const DE_WIRE_MAX: usize = 112_200;

/// The resync DE of tick `t`, made by the exe's own writers on the host's history:
/// 1. the full item writer 0x2002FC5A0 on the item with time == t;
/// 2. for each present player (lowest slot first) and each tail tick u from the first tail tick to t - tick: the
///    player's animation block of item u (writer 0x2002EF5D0) on the same stream, a zero byte when item u is missing;
/// 3. checks: item time == t, block count, size <= 0x44800 decoded and the relay's stored-LZMA2 payload <= 112 200.
pub fn de_with_tail(core: u64, opts: u64, t: u32) -> Result<DeBody, String> {
    static BUF: AtomicU64 = AtomicU64::new(0);
    unsafe {
        let find = |u: u32| -> u64 {
            let mut item = rd64(core + 0x8008);
            while item != 0 && *((item + 0x106238) as *const u32) > u {
                item = rd64(item + 0x106228);
            }
            if item != 0 && *((item + 0x106238) as *const u32) == u { item } else { 0 }
        };
        let item = find(t);
        if item == 0 {
            return Err(format!("no history item of time {t}"));
        }
        let mut pres = rd64(core + 0x16C4F0);
        while pres != 0 && *((pres + 0x18) as *const u32) > t {
            pres = rd64(pres + 8);
        }
        let mut b = BUF.load(Ordering::Relaxed);
        if b == 0 {
            b = mem::heap_alloc(DE_DECODED_MAX + 0x30, true) as u64;
            BUF.store(b, Ordering::Relaxed);
        }
        let (wr, inner, data) = (b, b + 0x10, b + 0x30);
        std::ptr::write_bytes(b as *mut u8, 0, 0x30);
        *(wr as *mut u8) = 0xFF;
        *((wr + 8) as *mut u64) = inner;
        *((inner + 8) as *mut u64) = data;
        *((inner + 0x10) as *mut u32) = DE_DECODED_MAX as u32;
        let (p10, p14) = if pres != 0 { (pres + 0x10, pres + 0x14) } else { (0, 0) };
        let writer: extern "win64" fn(u64, u64, u64, u64, u64, u8) -> u64 = std::mem::transmute(0x2002FC5A0u64);
        writer(item, wr, p10, p14, p14, 1);
        let pos = |_: ()| *((inner + 0x14) as *const u32) as usize;
        let item_len = pos(());
        let mask = if pres != 0 { *(p14 as *const u32) } else { 0 };
        let acc = *((opts + 0x107A) as *const u16) as i64;
        let tick = (*((opts + 0x1083) as *const u8) as u32).max(1);
        let first_tail_tick = if (t as i64) < acc {
            0
        } else {
            let x = t as i64 - acc - tick as i64;
            (x - x.rem_euclid(tick as i64) + tick as i64).max(0) as u32
        };
        let block: extern "win64" fn(u64, u64) = std::mem::transmute(0x2002EF5D0u64);
        let (mut blocks, mut missing) = (0u32, 0u32);
        let mut per_player = Vec::new();
        for p in 0..32u64 {
            if mask & (1 << p) == 0 {
                continue;
            }
            let start = pos(());
            let mut u = first_tail_tick;
            while u < t {
                if pos(()) + 0x10000 > DE_DECODED_MAX {
                    return Err(format!("DE of {t}: the stream reached {} bytes", pos(())));
                }
                let it = find(u);
                if it != 0 {
                    block(it + p * 0xCDA0 + 0x1550, wr);
                } else {
                    let at = pos(());
                    *((data + at as u64) as *mut u8) = 0;
                    *((inner + 0x14) as *mut u32) = at as u32 + 1;
                    missing += 1;
                }
                blocks += 1;
                u += tick;
            }
            per_player.push((p as u8, pos(()) - start));
        }
        let n = pos(());
        let bytes = std::slice::from_raw_parts(data as *const u8, n.min(DE_DECODED_MAX)).to_vec();
        let d = DeBody { t, item_len, mask, first_tail_tick, tick, blocks, missing, per_player, bytes };
        // the checks a relay could also make before handing the DE out
        let want = mask.count_ones() * ((t - first_tail_tick) / tick);
        if d.bytes.len() < 4 || u32::from_le_bytes(d.bytes[..4].try_into().unwrap()) != t {
            return Err(format!("DE of {t}: the item time is not {t}"));
        }
        if blocks != want {
            return Err(format!("DE of {t}: {blocks} tail blocks, expected {want}"));
        }
        if n > DE_DECODED_MAX {
            return Err(format!("DE of {t}: {n} bytes decoded (> {DE_DECODED_MAX})"));
        }
        let wire = n + 3 * n.div_ceil(0x10000) + 1;
        if wire > DE_WIRE_MAX {
            return Err(format!("DE of {t}: {wire} bytes on the wire (> {DE_WIRE_MAX})"));
        }
        Ok(d)
    }
}

/// Every address in the host's resident, writable mappings (low area and the image) that holds `pat`.
fn fnv64(b: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for &x in b {
        h = (h ^ x as u64).wrapping_mul(0x100000001b3);
    }
    h
}

/// Replay a SRPREC01 recording's DFs through [`crate::world::MatchWorld::step_df`]. Per tick the E0 data go to
/// <out>/ticks.tsv. `--replay-until T` stops early. Captures (the full = 1 writer output, the bytes of the client's
/// world-save capture): HOST_CAPTURE_EVERY=N / HOST_CAPTURE_UNTIL=<ms> select the ticks T with T/50 % N == 0 or
/// T <= ms; HOST_CAPTURE_DELAY=ms takes each after stepping T + ms (the final world with late inputs). HOST_MEM_AT
/// marks memory snapshots; the step times and the end results are logged.
fn replay(mw: &mut crate::world::MatchWorld, path: &str, out: &str, mut bots: Option<&mut crate::bots::HostBots>) -> Result<(), String> {
    let d = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    if d.len() < 18 || &d[..8] != b"SRPREC01" {
        return Err("not SRPREC01".into());
    }
    let n = d[17] as usize;
    let mut p = 18 + 2 * n;
    for _ in 0..2 {
        let l = u32::from_le_bytes(d[p..p + 4].try_into().map_err(|_| "header")?) as usize;
        p += 4 + l;
    }
    let a: Vec<String> = std::env::args().collect();
    let mut until: u32 = a.iter().position(|x| x == "--replay-until").and_then(|i| a.get(i + 1)).and_then(|v| v.parse().ok()).unwrap_or(u32::MAX);
    // stop at the last normal DF: the relay's 4 closing DFs (every recipient's leave bit) never reach a live host
    {
        let (mut q, mut times, mut close) = (p, Vec::new(), None);
        while q + 5 <= d.len() {
            let t = u32::from_le_bytes(d[q + 1..q + 5].try_into().unwrap());
            if d[q] == 0xFF {
                close = Some(t);
                break;
            }
            if q + 9 > d.len() {
                break;
            }
            times.push((t, ()));
            q += 9 + u32::from_le_bytes(d[q + 5..q + 9].try_into().unwrap()) as usize;
        }
        let fin = crate::daemon::fin_dfs(close, d[16], &times);
        if fin > 0 && !a.iter().any(|x| x == "--keep-fin-dfs") {
            let last = times[times.len() - fin - 1].0;
            hlog!("replay: the last {fin} DFs are the relay's closing DFs; stopping at DF {last}");
            until = until.min(last);
        }
    }
    let env = |k: &str| std::env::var(k).ok().and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
    let (every, until_ms, cap_delay) = (env("HOST_CAPTURE_EVERY"), env("HOST_CAPTURE_UNTIL"), env("HOST_CAPTURE_DELAY"));
    if every != 0 || until_ms != 0 {
        let _ = std::fs::create_dir_all(format!("{out}/capture"));
    }
    let mut de_at: Vec<u32> = std::env::var("HOST_DE_AT").ok().map(|v| v.split(',').filter_map(|x| x.parse().ok()).collect()).unwrap_or_default();
    de_at.sort_unstable();
    de_at.reverse();
    let mut mem_at: Vec<u32> = std::env::var("HOST_MEM_AT").ok().map(|v| v.split(',').filter_map(|x| x.parse().ok()).collect()).unwrap_or_default();
    mem_at.sort_unstable();
    mem_at.reverse();
    let mut tsv = String::from("T\titem\tmask\thash_per_mask_bit\tg0g1g2\tstream_len\tstream_fnv64\n");
    let mut step_times: Vec<(u32, u64, u64, u64)> = Vec::new();
    let mut counts = [0u64; 7];
    let mut last_state = mw.results().state;
    let mut last_values = team_values(mw.core);
    let mut nonzero = 0u64;
    let stack_trace = std::env::var("HOST_STACK_TRACE").is_ok_and(|v| v == "1");
    let mut prev_stacks: Vec<crate::world::PlayerStacks> = Vec::new();
    if let Some(b) = bots.as_deref_mut() {
        b.rec_header(&d[..p]);
    }
    // the records the replay does not step (the closing DFs, the end marker) go into the bots' recording copy as they are
    let mut tail_from = d.len();
    while p + 5 <= d.len() {
        tail_from = p;
        let kind = d[p];
        let t = u32::from_le_bytes(d[p + 1..p + 5].try_into().unwrap());
        while mem_at.last().is_some_and(|&m| t >= m) {
            let m = mem_at.pop().unwrap();
            mem_snapshot(&format!("replay at T>={m} (record time {t})"));
        }
        if kind == 0xFF {
            hlog!("replay: end marker, close tick {t}");
            break;
        }
        if p + 9 > d.len() {
            hlog!("replay: recording truncated at byte {p}");
            break;
        }
        let l = u32::from_le_bytes(d[p + 5..p + 9].try_into().unwrap()) as usize;
        if p + 9 + l > d.len() {
            hlog!("replay: recording truncated at byte {p} (record of {l} bytes at t {t})");
            break;
        }
        let body = &d[p + 9..p + 9 + l];
        p += 9 + l;
        if t > until {
            break;
        }
        tail_from = d.len();
        if let Some(b) = bots.as_deref_mut() {
            b.on_df(t, body);
        }
        let c0 = thread_cpu_ns();
        let w0 = std::time::Instant::now();
        let st = mw.step_df(body)?;
        let c1 = thread_cpu_ns();
        let w1 = w0.elapsed().as_nanos() as u64;
        for (c, x) in counts.iter_mut().zip(st.inserted) {
            *c += x as u64;
        }
        // the mode rule's two team values (S+0xEBDC / S+0xEBDE: tdm tickets, Search and Destroy rounds won): every change
        let tv = team_values(mw.core);
        if tv != last_values {
            hlog!("replay: team values S+0xEBDC/S+0xEBDE {last_values:?} -> {tv:?} at DF {} (rule state {})", st.t, st.state);
            last_values = tv;
        }
        if st.state != last_state {
            hlog!("replay: rule state W+0x180AE0 {last_state} -> {} at DF {}", st.state, st.t);
            if st.state == 4 || st.state == 5 {
                outcome_report(mw.core, mw.records(), &format!("state {} at {}", st.state, st.t));
            }
            last_state = st.state;
        }
        // the tick's E0 data: writer 0x2002FC5A0 full = 0, its hash bytes and stream (as the client's E0 producer)
        let e0 = tick_e0(mw.core, st.t, 0);
        step_times.push((st.t, c1 - c0, w1, thread_cpu_ns() - c1));
        if let Some((it, h, sb)) = e0 {
            let mask = h[20] as u32 | (h[21] as u32) << 8 | (h[22] as u32) << 16;
            let hb: Vec<u8> = (0..20).filter(|i| mask & (1u32 << i) != 0).map(|i| h[i]).collect();
            if hb.iter().any(|&x| x != 0) {
                nonzero += 1;
            }
            tsv.push_str(&format!(
                "{}\t{it}\t{mask:06x}\t{}\t{:02x}{:02x}{:02x}\t{}\t{:016x}\n",
                st.t,
                if hb.is_empty() { "-".to_string() } else { hb.iter().map(|b| format!("{b:02x}")).collect::<String>() },
                h[23],
                h[24],
                h[25],
                sb.len(),
                fnv64(&sb)
            ));
        }
        // HOST_DE_AT=T1,T2,...: the resync DE of each T once T is final (DF T + 300 stepped): de_<T>.bin + its sizes
        while de_at.last().is_some_and(|&x| st.t >= x + 300) {
            let x = de_at.pop().unwrap();
            match mw.de(x) {
                Ok(d) => {
                    let _ = std::fs::write(format!("{out}/de_{x}.bin"), &d.bytes);
                    hlog!(
                        "DE {x}: {} bytes = item {} + tail {} (mask {:#x}, first tail tick {}, tick {}, {} blocks, {} missing); tail per player {:?}",
                        d.bytes.len(),
                        d.item_len,
                        d.bytes.len() - d.item_len,
                        d.mask,
                        d.first_tail_tick,
                        d.tick,
                        d.blocks,
                        d.missing,
                        d.per_player
                    );
                }
                Err(e) => hlog!("DE {x}: refused: {e}"),
            }
        }
        let ct = st.t.wrapping_sub(cap_delay);
        if cap_delay <= st.t && (ct <= until_ms || (every != 0 && (ct / 50) % every == 0)) {
            if let Some((it, b)) = mw.full_item(ct) {
                let _ = std::fs::write(format!("{out}/capture/full_{it}.bin"), &b);
            }
        }
        // HOST_STACK_TRACE=1 (diagnosis): every change of a human's stacks (storage, carried), alive flag or profile, and
        // every late input (the world simulates again from late_from): the ammo-charge rollback check
        if stack_trace {
            if let Some(l) = st.late_from {
                hlog!("stacks: DF {} late input from {l} (present before {})", st.t, st.t);
            }
            let now: Vec<crate::world::PlayerStacks> = mw.stacks().into_iter().filter(|p| p.human).collect();
            for p in &now {
                if prev_stacks.iter().find(|q| q.slot == p.slot) != Some(p) {
                    let s: Vec<String> = p.stacks.iter().map(|x| format!("{}/{}:{}+{}", x.uid, x.dict, x.storage, x.carried)).collect();
                    hlog!("stacks: DF {} slot {} alive {} profile {} {}", st.t, p.slot, p.alive, p.profile, s.join(" "));
                }
            }
            prev_stacks = now;
        }
        // host bots: the AI tick after the step and after the tick's E0 and capture (the same moment a host without
        // the AI sees the world of DF t)
        if let Some(b) = bots.as_deref_mut() {
            b.after_step(st.t);
        }
    }
    match mw.used() {
        Some(players) => {
            for p in &players {
                hlog!("replay: used: slot {} profiles {:#x} weapons {:?} stacks {:?}", p.slot, p.profiles, p.weapons, p.stacks);
            }
        }
        None => hlog!("replay: used: no DF stepped"),
    }
    if let Some(b) = bots.as_deref_mut() {
        b.rec_tail(&d[tail_from..]);
        b.report(Some(&mw.results()));
        b.flush();
    }
    crate::bots::log_voice_and_asserts();
    let _ = std::fs::write(format!("{out}/ticks.tsv"), &tsv);
    step_time_report(&step_times, out);
    outcome_report(mw.core, mw.records(), "end of the replay");
    crate::bp::memcpy_report(out);
    hlog!(
        "replay: {} DFs stepped (last {}); inserted enter {}, leave {}, player list1 {}, list2 {}, profile {}, ping {}, ai {}; change_input non-zero returns {}, inputs at or before present {}; ticks with non-zero hash bytes {nonzero}",
        mw.steps, mw.last_t, counts[0], counts[1], counts[2], counts[3], counts[4], counts[5], counts[6], mw.rejected, mw.rolled_back
    );
    hlog!("replay: render commands retired {} ({} with the consumer's keep marks), render pool peak {} of 8192", mw.render_retired, mw.render_marked, mw.render_pool_peak);
    Ok(())
}

/// The commands waiting in the render world's one_way_render_channel (no :render thread consumes them). Walks the
/// node list from the consumer end (ch+8) through each node's 44-bit next field (node+0x10; the bits above 44 hold a
/// counter) and counts the nodes per vtable class.
pub fn render_channel_report(label: &str) {
    unsafe {
        let ch = 0x2034C3D90u64;
        let mut n = rd64(ch + 8);
        if n == 0 {
            hlog!("render channel [{label}]: not initialised");
            return;
        }
        let mut count = 0u64;
        let mut kinds: std::collections::BTreeMap<String, u64> = std::collections::BTreeMap::new();
        while n != 0 && count < 10_000_000 {
            count += 1;
            *kinds.entry(rtti_name(rd64(n)).trim_start_matches(".?A").to_string()).or_insert(0) += 1;
            n = rd64(n + 0x10) & 0xFFF_FFFF_FFFF;
        }
        let mut v: Vec<_> = kinds.into_iter().collect();
        v.sort_by(|a, b| b.1.cmp(&a.1));
        let top: Vec<String> = v.iter().take(6).map(|(k, c)| format!("{c} {k}")).collect();
        hlog!("render channel [{label}]: {count} node(s) queued (incl. the sentinel); {}", top.join(", "));
    }
}

/// `--bp-log A,B` (logged exec breakpoints) and `--watch A,B` (8-byte write watchpoints), armed on every thread when
/// step 3 reaches `--debug-at` (project | match, default match). Addresses in hex.
fn arm_debug(stage: &str) {
    let a: Vec<String> = std::env::args().collect();
    let get = |k: &str| a.iter().position(|x| x == k).and_then(|i| a.get(i + 1)).cloned();
    if get("--debug-at").as_deref().unwrap_or("match") != stage {
        return;
    }
    let list = |k: &str| -> Vec<u64> {
        get(k).map(|v| v.split(',').filter_map(|x| u64::from_str_radix(x.trim_start_matches("0x"), 16).ok()).collect()).unwrap_or_default()
    };
    if let Some((f, t)) = std::env::var("HOST_CC_DUMP").ok().and_then(|v| {
        let (a, b) = v.split_once(',')?;
        Some((a.parse::<u32>().ok()?, b.parse::<u32>().ok()?))
    }) {
        let out = get("--out").unwrap_or(DEFAULT_OUT.into());
        crate::bp::arm_cc(&out, f, t);
    }
    crate::bp::arm_armw();
    crate::bp::arm_memcpy_log();
    crate::bp::arm_query_log();
    let (l, w) = (list("--bp-log"), list("--watch"));
    if !l.is_empty() {
        crate::bp::arm_log(&l);
    }
    if !w.is_empty() {
        unsafe {
            for &x in &w {
                hlog!("watch: [{x:#x}] = {:#x} when armed ({stage})", *(x as *const u64));
            }
        }
        crate::bp::set_watch_time();
        crate::bp::arm_watch(&w);
    }
}

/// The match results a relay would take from the host (the Outcome / Stats messages): rule state `W+0x180AE0` (4 / 5 =
/// ended), statistics object S = `[W+0x180A38]` (vtable 0x20096FE88, core = S+8): winner team `S+0xEBC0` (3 = none),
/// two synced u16 `S+0xEBD4` / `S+0xEBD6` (meaning OPEN), present mask `S+0xEBC8`; per player block
/// `B = S+0x30 + i*0xA24`: `u16 count[74 events][9 profiles]` (event e at B + e*0x12; kill 0, death 2, pve_open_chest
/// 42, which the HUD chest counter sums over the players), score `u16 B+0xA1C`. Read-only.
pub fn outcome_report(core: u64, n_players: u32, label: &str) {
    unsafe {
        let w = core + 0x230;
        let state = *((w + 0x180AE0) as *const u32);
        let s = rd64(w + 0x180A38);
        if s == 0 {
            hlog!("OUTCOME [{label}] world state {state}, no statistics object");
            return;
        }
        let winner = rd8(s + 0xEBC0);
        let t4 = *((s + 0xEBD4) as *const u16);
        let t6 = *((s + 0xEBD6) as *const u16);
        let present = *((s + 0xEBC8) as *const u32);
        let row = |b: u64, e: u64| -> u32 { (0..9u64).map(|p| *((b + e * 0x12 + p * 2) as *const u16) as u32).sum() };
        let mut per = Vec::new();
        let mut chests = 0;
        for i in 0..n_players.min(20) as u64 {
            let b = s + 0x30 + i * 0xA24;
            let (k, d, c, sc) = (row(b, 0), row(b, 2), row(b, 42), *((b + 0xA1C) as *const u16));
            // Search and Destroy events: 47 sad_bomb_planted, 48 sad_bomb_defuse, 49 sad_round_won
            let (pl, df, rw) = (row(b, 47), row(b, 48), row(b, 49));
            chests += c;
            if k + d + c + sc as u32 + pl + df + rw != 0 {
                per.push(format!("p{i} k{k} d{d} c{c} s{sc}{}", if pl + df + rw != 0 { format!(" planted{pl} defused{df} rounds_won{rw}") } else { String::new() }));
            }
        }
        hlog!(
            "OUTCOME [{label}] world time {} state {state} winner {winner} S+0xEBD4 {t4} S+0xEBD6 {t6} team values {:?} present {present:#x}; chests (event 42) {chests}; players: {}",
            *((w + 0x180A98) as *const u32),
            team_values(core),
            per.join(", ")
        );
    }
}

/// The mode rule's two team values u16 `S+0xEBDC` / `S+0xEBDE` (world.rs `Results::team_score`; Search and Destroy:
/// rounds won, 0x2003AE229); (0, 0) without a statistics object. Read-only.
pub fn team_values(core: u64) -> (u16, u16) {
    unsafe {
        let s = rd64(core + 0x230 + 0x180A38);
        if s == 0 {
            return (0, 0);
        }
        (*((s + 0xEBDC) as *const u16), *((s + 0xEBDE) as *const u16))
    }
}

/// The fork()ed child's number (1..N) in --fork-match mode, 0 in the parent / without it.
static FORK_CHILD: AtomicU64 = AtomicU64::new(0);
/// --fork-match: fork n children at once. Returns true in the parent (after all children ended), false in a child.
/// The child keeps the parent's world thread id (REPORTED_TID: the TEB's id field, the cooks' +0x18 thread ids and the
/// exe's thread registrations all hold the parent's id). HOST_FORK_TIMEOUT (s, default 600) kills late children.
fn fork_children(n: u32) -> bool {
    mem_snapshot("parent before fork (match)");
    mem_breakdown("parent before fork (match)");
    crate::log::flush();
    let mut pids = Vec::new();
    for k in 1..=n {
        let pid = fork_world();
        if pid == 0 {
            FORK_CHILD.store(k as u64, Ordering::Relaxed);
            mem_snapshot(&format!("child {k}: right after fork"));
            return false;
        }
        if pid < 0 {
            hlog!("fork {k} failed: {}", std::io::Error::last_os_error());
            break;
        }
        pids.push((k, pid));
    }
    let limit: u64 = std::env::var("HOST_FORK_TIMEOUT").ok().and_then(|v| v.parse().ok()).unwrap_or(600);
    let t0 = std::time::Instant::now();
    while !pids.is_empty() {
        pids.retain(|&(k, pid)| {
            let mut st = 0;
            let r = unsafe { libc::waitpid(pid, &mut st, libc::WNOHANG) };
            if r == pid {
                hlog!("fork match: child {k} (pid {pid}) exited, status {st:#x}, after {} ms", t0.elapsed().as_millis());
                return false;
            }
            if t0.elapsed().as_secs() > limit {
                unsafe { libc::kill(pid, libc::SIGKILL) };
                hlog!("fork match: child {k} (pid {pid}) killed after {limit} s");
                return false;
            }
            true
        });
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    true
}

/// fork() the world thread's process (only this thread survives). Returns fork()'s value; in the child (0) the exe
/// threads it needs again run: the child keeps the parent's world thread id (FORCED_TID: the TEB's id field, the
/// cooks' +0x18 thread ids and the exe's thread registrations hold it), the crash handler's thread list holds only this
/// thread, and the exe threads in HOST_FORK_RESTART (default ":hdd,:resources manager") are started again with their
/// worker-slot jobs (table 0x200AB1A00, 0x40 bytes each: +0 boost::function, +0x30 thread id, +0x38 posted flag)
/// posted again.
pub fn fork_world() -> i32 {
    let world_tid = crate::k32::GetCurrentThreadIdPub();
    let pid = unsafe { libc::fork() };
    if pid != 0 {
        return pid;
    }
    crate::k32::FORCED_TID.with(|t| t.set(world_tid));
    if let Ok(mut v) = crate::crash::TIDS.lock() {
        v.clear();
        v.push(crate::teb::gettid() as i32);
    }
    hlog!("child pid {} after fork; reporting world thread id {world_tid}", unsafe { libc::getpid() });
    let list = std::env::var("HOST_FORK_RESTART").unwrap_or(":hdd,:resources manager".into());
    for name in list.split(',').filter(|x| !x.is_empty()) {
        if let Some(old) = crate::k32::restart_exe_thread(name) {
            unsafe {
                let t = rd64(0x200AB1A00);
                for i in 0..16u64 {
                    let e = t + i * 0x40;
                    if *((e + 0x30) as *const u32) as u64 == old && rd64(e) != 0 {
                        (*((e + 0x38) as *const std::sync::atomic::AtomicU32)).store(1, Ordering::SeqCst);
                        hlog!("child: worker slot {i} ({name}): job {:#x} posted again", rd64(e));
                    }
                }
            }
        }
    }
    0
}

/// This thread's CPU time in ns (CLOCK_THREAD_CPUTIME_ID).
fn thread_cpu_ns() -> u64 {
    let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut ts) };
    ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64
}
/// Mean / p50 / p99 / max of the per-tick world step (move) and E0 (writer + hash) times; steptimes.tsv per tick.
fn step_time_report(v: &[(u32, u64, u64, u64)], out: &str) {
    if v.is_empty() {
        return;
    }
    let stats = |mut x: Vec<u64>| -> String {
        x.sort_unstable();
        let n = x.len();
        let mean = x.iter().sum::<u64>() as f64 / n as f64;
        let q = |f: f64| x[((n as f64 * f) as usize).min(n - 1)];
        format!("mean {:.3} ms, p50 {:.3}, p99 {:.3}, max {:.3} ms", mean / 1e6, q(0.5) as f64 / 1e6, q(0.99) as f64 / 1e6, x[n - 1] as f64 / 1e6)
    };
    let mv: Vec<u64> = v.iter().map(|e| e.1).collect();
    let wall: Vec<u64> = v.iter().map(|e| e.2).collect();
    let e0: Vec<u64> = v.iter().map(|e| e.3).collect();
    let both: Vec<u64> = v.iter().map(|e| e.1 + e.3).collect();
    let mean_both = both.iter().sum::<u64>() as f64 / both.len() as f64;
    hlog!("STEPTIME {} ticks: move CPU {}", v.len(), stats(mv));
    hlog!("STEPTIME move wall {}", stats(wall));
    hlog!("STEPTIME E0 writer+hash CPU {}", stats(e0));
    hlog!("STEPTIME move+E0 CPU {}; at 20 ticks/s = {:.2} % of one core", stats(both), mean_both / 1e6 * 20.0 / 10.0);
    let mut t = String::from("time\tmove_cpu_ns\tmove_wall_ns\te0_cpu_ns\n");
    for e in v {
        t.push_str(&format!("{}\t{}\t{}\t{}\n", e.0, e.1, e.2, e.3));
    }
    let _ = std::fs::write(format!("{out}/steptimes.tsv"), t);
}

fn world_thread(o: Opts) -> i32 {
    let exe = o.exe.clone();
    let l = match load(&exe) {
        Ok(l) => l,
        Err(e) => {
            hlog!("load failed: {e}");
            return 2;
        }
    };
    mem_snapshot("after load");
    if let Err(e) = step1(&l, &o) {
        hlog!("STEP 1 FAILED: {e}");
        return 10;
    }
    hlog!("=== step 1 finished");
    if let Some(ms) = o.linger_ms {
        std::thread::sleep(std::time::Duration::from_millis(ms));
        hlog!("lingered {ms} ms");
    }
    if o.steps < 2 {
        return 0;
    }
    if let Err(e) = step2(&l, &o) {
        hlog!("STEP 2 FAILED: {e}");
        return 20;
    }
    hlog!("=== step 2 finished");
    if o.steps >= 3 {
        if let Err(e) = step3(&o) {
            hlog!("STEP 3 FAILED: {e}");
            return 30;
        }
        hlog!("=== step 3 finished");
    }
    if let Some(ms) = o.linger_ms {
        std::thread::sleep(std::time::Duration::from_millis(ms));
        hlog!("lingered {ms} ms");
    }
    0
}

fn auto_level(m: Option<String>) -> Result<String, String> {
    let gd = crate::run::game_tables()?;
    let project = match std::env::var("HOST_PVE").ok().and_then(|v| v.split_once(':').and_then(|(d, m)| Some((d.parse::<u8>().ok()?, m.parse::<u8>().ok()?)))) {
        Some((d, mz)) => gd.pve.project(d, mz).ok_or("no PvE project")?.to_string(),
        None => {
            let (map, mode) = m.as_deref().and_then(|s| s.split_once(',')).ok_or("--match MAP,MODE needed")?;
            let (map, mode) = (map.parse().map_err(|_| "map")?, mode.parse().map_err(|_| "mode")?);
            (0..16u32).filter_map(|v| gd.matches.resolve_map(map, mode, v).filter(|r| v == 0 || r.variation == v)).next().ok_or("no mode row")?.project.clone()
        }
    };
    let i = project.match_indices('_').nth(1).map(|(i, _)| i).ok_or(format!("project '{project}' has no second '_'"))?;
    Ok(format!("{}/{}", &project[..i], &project[i + 1..]))
}

pub fn main() -> i32 {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("daemon") {
        return crate::daemon::front_main(&args[2..]);
    }
    if args.get(1).map(String::as_str) == Some("relay-replay") {
        return crate::daemon::relay_replay_main(&args[2..]);
    }
    if args.get(1).map(String::as_str) == Some("prewarm-list") {
        return crate::daemon::prewarm_list_main(&args[2..]);
    }
    // `world-host perf-probe`: can this process arm hardware breakpoints (the kill log needs them)? srp selftest asks
    if args.get(1).map(String::as_str) == Some("perf-probe") {
        return match crate::bp::probe() {
            Ok(()) => {
                println!("ok");
                0
            }
            Err(e) => {
                println!("{e}");
                1
            }
        };
    }
    let get =|k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    // the exe's path here and as the game sees it, e.g. /path/to/game/binaries/x64/survarium.exe and
    // C:\Games\Survarium\binaries\x64\survarium.exe
    let (Some(exe), Some(exe_win)) = (get("--exe"), get("--exe-win")) else {
        eprintln!("usage: world-host --exe <survarium.exe> --exe-win <its Windows path> [--out DIR] [options] (or: world-host daemon ...)");
        return 2;
    };
    let out = PathBuf::from(get("--out").unwrap_or(DEFAULT_OUT.into()));
    let steps: u32 = get("--steps").and_then(|s| s.parse().ok()).unwrap_or(1);
    let fma = get("--fma").map(|s| u32::from_str_radix(s.trim_start_matches("0x"), 16).unwrap());
    let mut level = get("--level").unwrap_or("level_07_day".into());
    if level == "auto" {
        // `--level auto` with `--match MAP,MODE` (or HOST_PVE=d:m): the options' project with its second '_' as '/'
        // (level_01_day_tdm -> level_01/day_tdm, pve_01_v1_maze2 -> pve_01/v1_maze2)
        level = auto_level(get("--match")).unwrap_or_else(|e| {
            eprintln!("--level auto: {e}");
            std::process::exit(2)
        });
    }
    let cores: u32 = get("--cores").and_then(|s| s.parse().ok()).unwrap_or(2);
    let cmdline_extra = get("--args").unwrap_or("-product_name=Survarium-Steam -localization=english -firewall_check=0 -splash_screen=0".into());
    let _ = END_THREADS.set(get("--end-threads").map(|s| s.split(',').map(|x| x.to_string()).collect()).unwrap_or_default());
    let _ = END_AT.set(
        get("--end-at")
            .unwrap_or("ole32.dll,WS2_32.dll,VerifyVersionInfoA,IPHLPAPI.DLL,qwave.dll".into())
            .split(',')
            .map(|x| x.to_string())
            .collect(),
    );
    let _ = ESCAPE_AT.set(get("--escape-at").map(|s| s.split(',').map(|x| x.to_string()).collect()).unwrap_or_default());
    crate::k32::DEBUGGER.store(!args.iter().any(|a| a == "--no-debugger"), Ordering::Relaxed);
    crate::k32::NO_XAUDIO.store(args.iter().any(|a| a == "--no-xaudio"), Ordering::Relaxed);
    VERBOSE_MEM.store(args.iter().any(|a| a == "--verbose-mem"), Ordering::Relaxed);
    let _ = std::fs::create_dir_all(&out);
    crate::log::open(out.join("host.log").to_str().unwrap());
    let _ = OUT.set(out.clone());
    if args.iter().any(|a| a == "--quiet") {
        crate::log::set_quiet_dbg(true);
    }
    hlog!("world-host: exe {exe}, out {}", out.display());
    let cwd_win = exe_win.rsplit_once('\\').map(|(d, _)| d.to_string()).unwrap();
    let game = crate::files::game_mapping(std::path::Path::new(&exe), &exe_win);
    if let Some((w, u)) = &game {
        hlog!("game folder {w} = {}", u.display());
    }
    let _ = CFG.set(FsConfig { cwd_win, overlay: out.join("overlay"), zdrive: out.join("zdrive"), game });
    let _ = PROC.set(ProcCfg { exe_win: exe_win.clone(), cmdline: format!("\"{exe_win}\" {cmdline_extra}"), cores });
    crate::crash::install();
    let linger_ms = get("--linger").and_then(|s| s.parse().ok());
    let pump_ms = get("--pump-ms").and_then(|s| s.parse().ok()).unwrap_or(3000);
    let o = Opts { exe, exe_win, out, steps, fma, level, cmdline_extra, linger_ms, pump_ms };
    // the world thread runs exe code: its stack must be below 2^44 as well
    let (tx, rx) = std::sync::mpsc::channel();
    let t = mem::spawn_low(256 << 20, Box::new(move || {
        let _ = tx.send(world_thread(o));
    }));
    unsafe { libc::pthread_join(t, std::ptr::null_mut()) };
    let code = rx.recv().unwrap_or(99);
    write_stub_report();
    hlog!("world-host exit {code}");
    crate::log::flush();
    // exe threads may still run: leave without destructors
    unsafe { libc::_exit(code) }
}
