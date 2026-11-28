//! Null cooks: a world host has no render device, no sound and no particles. A null cook replaces the cook of one
//! resource class in the resource manager's cook table (0x200AAF1A0, index = class at cook+0xC). Its translate_query
//! (slot 10) answers every request at once with an empty resource: a zeroed object whose vtable is a host table of
//! logging no-ops, stored with the exe's own `set_result` 0x2000BFD10 before the query is finished the way the exe's
//! own cooks finish it.
//! Every request is logged once per class with its name, so the report can list what was answered empty.
#![allow(non_snake_case)]

use crate::hlog;
use std::collections::BTreeMap;
use std::sync::Mutex;

/// The object size an empty resource reports to the exe's memory statistics (set_result's descriptor): 0x1000 for
/// every class, the value it was before the per-class slots, so nothing the exe sees changes.
const NULL_RES_STAT_SIZE: u32 = 0x1000;

/// Bytes of the empty resource of `class` (a zeroed slot of `mem::pool_alloc`). Where the exe's own cook allocates a
/// fixed object size for the class, the slot is that size rounded up to 16 (PROVEN: allocation sites below): correct
/// exe code never reaches past the real object's size. The classes with a variable-length tail get their fixed part:
/// 0x13 0x3E8 (allocation 0x2004DAE68: 0x3E8 + n * 0x1A8; set_result passes 0x3E8, 0x2004DB095), 0x101 0x238
/// (0x20063260A: 0x238 + n * 0x40), 0x102 0x240 (0x200631030: 0x240 + n * 0x20), 0x79 0x350 (0x2004F74F9: 0x350 plus
/// per-cell arrays). Their tails are filled only by the real cooks (never run in the host) and reached only through
/// the object's own pointers and counts, which are 0 on a stand-in: PROVEN for the cooks, and for every other reader by
/// the guard-page run (HOST_NULL_RES_GUARD=right at 0x3F0 / 0x240 / 0x240 / 0x310 on every served project's level load
/// and match creation and through the regression recordings: no access past those sizes). 0x13 is the most frequent
/// class (one per render visual: about 1 000-2 000 in a PvP level, 7 190 in a PvE mission). Class 0x14's real object is
/// 0x5D58 bytes and exe render code writes past +0x1000 of it (0x2004B95D0, in a render job the host never runs): it
/// gets its full size in whole pages, of which only page 0 is touched (the others cost no memory). Any other class
/// keeps one page.
/// Every slot covers the fields the null path itself writes (refcount +0x1B0, base ctor +0x160..+0x200, add_model
/// +0x210, placeable_item_set +0x218 on class 0x13).
fn null_res_size(class: u32) -> usize {
    match class {
        0x6 => 0x490,   // texture: 0x488 (alloc 0x20046E208)
        0xD => 0x360,   // material_effects_instance: 0x358 (0x20046BFDE)
        0x13 => 0x3F0,  // static_render_model_instance: 0x3E8 + n * 0x1A8 (0x2004DAE68), the fixed part
        0x14 => 0x6000, // skeleton_render_model_instance: 0x5D58 + n * 0x1A8 (0x2004E48E7), whole pages
        0x18 => 0x3F0,  // user_render_model_instance: 0x3E8 (0x2004DC603)
        0x3D => 0x600,  // particle_system_instance: 0x600 (0x2005D9D9B / 0x2005D9DDE)
        0x7B => 0x230,  // camouflage_decal_baker: 0x228 (0x2004602E3)
        0x97 => 0x240,  // load_shadow_proxy_geometry: 0x238 (0x2004F4227)
        0x100 => 0x240, // single_sound: 0x238 (0x20063390E)
        0x79 => 0x350,  // grass_world: 0x350 + per-cell arrays (0x2004F74F9), the fixed part
        0x101 => 0x240, // composite_sound: 0x238 + n * 0x40 (0x20063260A), the fixed part
        0x102 => 0x240, // sound_collection: 0x240 + n * 0x20 (0x200631030), the fixed part
        _ => 0x1000,
    }
}

static COUNTS: Mutex<BTreeMap<u32, u64>> = Mutex::new(BTreeMap::new());

/// Every empty resource this process (and the zygote it was forked from) made: address, size, class; recorded only
/// for the diagnosis switches (`pagemap`, HOST_NULL_RES_GUARD). The tag is the class with bit 31 set, and bit 30 for
/// a guarded one.
static STANDINS: crate::pagemap::Records<65536> = crate::pagemap::Records::new();
const STANDIN_TAG: u32 = 1 << 31;
const GUARDED_TAG: u32 = 1 << 30;

/// The recorded empty resources: (address, size, class).
pub fn standins() -> Vec<(usize, usize, u32)> {
    STANDINS.all().into_iter().map(|(a, n, t)| (a, n, t & !(STANDIN_TAG | GUARDED_TAG))).collect()
}

// ---------------------------------------------------------------- HOST_NULL_RES_GUARD (diagnosis only)
// Settles whether exe code reads or writes an empty resource past a given size (the tails of 0x13 / 0x79 / 0x101 /
// 0x102, class 0x14 past its first page). `right[,CLASS=SIZE...]`: every empty resource of class c is put at the end of
// its own slot (`mem::pool_alloc_guarded`) with an inaccessible page right after its `guard_size(c)` bytes and
// `mem::GUARD_FILL` in the bytes before it; an access past the size faults at once, and the fault handler names the
// class, the stand-in, the offset and the exe address (`guard_fault_note`); at zygote ready and at a match's Ready
// and end the bytes before every stand-in are checked (`guard_report`). `scan`: the normal layout; the same points log
// the highest non-zero byte of the stand-ins per class (writes only). Without the switch nothing changes.

/// The guard switch's mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuardMode {
    Off,
    Right,
    Scan,
}

static GUARD_MODE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
static GUARD_SIZES: std::sync::OnceLock<Vec<(u32, usize)>> = std::sync::OnceLock::new();
static GUARD_FAULTS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub fn guard_mode() -> GuardMode {
    match GUARD_MODE.load(std::sync::atomic::Ordering::Relaxed) {
        1 => GuardMode::Right,
        2 => GuardMode::Scan,
        _ => GuardMode::Off,
    }
}

/// The smallest size the null path needs (it writes up to +0x219: the 0x13 flag byte of 0x200241A4D).
const GUARD_MIN: usize = 0x220;

/// `HOST_NULL_RES_GUARD`: `right`, `right,0x13=0x1000,0x102=0x1600` (sizes for classes, hex), `scan`, or `off` / `0` /
/// empty.
pub fn parse_guard(spec: &str) -> Result<(GuardMode, Vec<(u32, usize)>), String> {
    let mut it = spec.split(',').map(str::trim);
    let mode = match it.next().unwrap_or("") {
        "right" => GuardMode::Right,
        "scan" => GuardMode::Scan,
        "" | "0" | "off" => GuardMode::Off,
        m => return Err(format!("unknown mode '{m}' (right, scan or off)")),
    };
    let hex = |s: &str| u64::from_str_radix(s.trim_start_matches("0x").trim_start_matches("0X"), 16).ok();
    let mut sizes = Vec::new();
    for part in it.filter(|p| !p.is_empty()) {
        if mode != GuardMode::Right {
            return Err(format!("'{part}': class sizes go with the mode right only"));
        }
        let (c, s) = part.split_once('=').ok_or(format!("'{part}': CLASS=SIZE expected"))?;
        let (Some(c), Some(s)) = (hex(c).and_then(|c| u32::try_from(c).ok()), hex(s).and_then(|s| usize::try_from(s).ok())) else {
            return Err(format!("'{part}': hex numbers expected"));
        };
        if !(GUARD_MIN..=0x10_0000).contains(&s) || s % 16 != 0 {
            return Err(format!("'{part}': a size from {GUARD_MIN:#x} to 0x100000, a multiple of 16"));
        }
        sizes.retain(|&(x, _)| x != c);
        sizes.push((c, s));
    }
    Ok((mode, sizes))
}

/// The size a guarded run gives class `class` unless the switch names one: the sizes the guard-page run checked for
/// the classes whose real tail is reached only through the object's own pointers and counts (0x13 0x3F0 = its fixed
/// part 0x3E8, 0x79 0x310, 0x101 / 0x102 0x240; the slots in use are these or larger) and class 0x14 with one page
/// only (its real object is 0x5D58 bytes); every other class its normal slot ([`null_res_size`]).
pub fn guard_default_size(class: u32) -> usize {
    match class {
        0x13 => 0x3F0,
        0x14 => 0x1000,
        0x79 => 0x310,
        0x101 | 0x102 => 0x240,
        c => null_res_size(c),
    }
}

/// The guarded size of `class` (the switch's own size first).
pub fn guard_size(class: u32) -> usize {
    GUARD_SIZES.get().and_then(|v| v.iter().find(|&&(c, _)| c == class).map(|&(_, s)| s)).unwrap_or_else(|| guard_default_size(class))
}

/// Reads `HOST_NULL_RES_GUARD` once at process start; the line to log when it is set (also when it is wrong: then off).
pub fn guard_init_from_env() -> Option<String> {
    let spec = std::env::var("HOST_NULL_RES_GUARD").ok()?;
    match parse_guard(&spec) {
        Ok((mode, sizes)) => {
            let note = match mode {
                GuardMode::Off => "off".to_string(),
                GuardMode::Right => {
                    let own: Vec<String> = sizes.iter().map(|(c, s)| format!("{c:#x} {s:#x}")).collect();
                    format!(
                        "every empty resource at the end of its slot with an inaccessible page after it; sizes 0x13 {:#x}, 0x14 {:#x}, 0x79 {:#x}, 0x101 {:#x}, 0x102 {:#x}, other classes their slot{}",
                        guard_default_size(0x13),
                        guard_default_size(0x14),
                        guard_default_size(0x79),
                        guard_default_size(0x101),
                        guard_default_size(0x102),
                        if own.is_empty() { String::new() } else { format!("; set here: {}", own.join(", ")) }
                    )
                }
                GuardMode::Scan => "the normal layout; the highest non-zero byte per class is logged".to_string(),
            };
            let _ = GUARD_SIZES.set(sizes);
            GUARD_MODE.store(
                match mode {
                    GuardMode::Off => 0,
                    GuardMode::Right => 1,
                    GuardMode::Scan => 2,
                },
                std::sync::atomic::Ordering::Relaxed,
            );
            Some(format!("HOST_NULL_RES_GUARD={spec}: {note}"))
        }
        Err(e) => Some(format!("HOST_NULL_RES_GUARD={spec} not taken ({e}): off")),
    }
}

/// The fault handler's line when `addr` is in the inaccessible page after a guarded stand-in (None otherwise, and
/// always without the switch). Reads the record table only (no lock).
pub fn guard_fault_note(addr: usize, ip: usize) -> Option<String> {
    if guard_mode() != GuardMode::Right {
        return None;
    }
    let page = crate::mem::PAGE;
    // the guarded stand-in whose inaccessible page holds addr (address, size, class), the stand-in after addr
    let mut hit: Option<(usize, usize, u32)> = None;
    let mut next: Option<(usize, u32)> = None;
    for i in 0..STANDINS.count() {
        let Some((a, n, t)) = STANDINS.get(i) else { continue };
        let c = t & !(STANDIN_TAG | GUARDED_TAG);
        if t & GUARDED_TAG != 0 && (a + n..a + n + page).contains(&addr) {
            hit = Some((a, n, c));
        }
        if a > addr && next.is_none_or(|(b, _)| a < b) {
            next = Some((a, c));
        }
    }
    let (a, n, c) = hit?;
    GUARD_FAULTS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let exe = if (0x2_0000_0000..0x2_15E3_8000).contains(&ip) { "exe code" } else { "not exe code" };
    let after = next.map_or(String::new(), |(b, d)| format!("; the next stand-in {b:#x} (class {d:#x}) starts {:#x} bytes after the address", b - addr));
    Some(format!("null guard fault: class {c:#x} stand-in {a:#x} (size {n:#x}) accessed at +{:#x}, from exe address {ip:#x} ({exe}){after}", addr - a))
}

/// The offset of the last non-zero byte of `b`.
pub fn last_nonzero(b: &[u8]) -> Option<usize> {
    b.iter().rposition(|&x| x != 0)
}

/// The guard switch's lines (nothing without it): `right` checks the fill before every guarded stand-in (a write there
/// is an access before the object) and counts the faults a guarded call survived; `scan` logs per class the highest
/// non-zero byte of its stand-ins.
pub fn guard_report(label: &str) {
    let mode = guard_mode();
    if mode == GuardMode::Off {
        return;
    }
    let page = crate::mem::PAGE;
    /// per class: stand-ins, the size of the last one, the highest non-zero byte and its stand-in
    #[derive(Default)]
    struct PerClass {
        n: usize,
        size: usize,
        high: Option<(usize, usize)>,
    }
    let mut per: BTreeMap<u32, PerClass> = BTreeMap::new();
    let (mut guarded, mut under, mut first_under) = (0usize, 0usize, None);
    for i in 0..STANDINS.count() {
        let Some((a, n, t)) = STANDINS.get(i) else { continue };
        let c = t & !(STANDIN_TAG | GUARDED_TAG);
        let e = per.entry(c).or_default();
        e.n += 1;
        e.size = n;
        match mode {
            GuardMode::Right if t & GUARDED_TAG != 0 => {
                guarded += 1;
                let first = a & !(page - 1);
                // SAFETY: the fill [first, a) is the accessible start of the stand-in's own slot
                let fill = unsafe { std::slice::from_raw_parts(first as *const u8, a - first) };
                let bad = fill.iter().filter(|&&x| x != crate::mem::GUARD_FILL).count();
                if bad > 0 && first_under.is_none() {
                    first_under = Some((c, a));
                }
                under += bad;
            }
            GuardMode::Scan => {
                // SAFETY: a stand-in's n bytes are its own readable slot
                if let Some(o) = last_nonzero(unsafe { std::slice::from_raw_parts(a as *const u8, n) })
                    && e.high.is_none_or(|(m, _)| o > m)
                {
                    e.high = Some((o, a));
                }
            }
            _ => {}
        }
    }
    let classes: Vec<String> = per
        .iter()
        .map(|(c, e)| match mode {
            GuardMode::Scan => format!("{c:#x} {} of {:#x}, highest non-zero byte {}", e.n, e.size, e.high.map_or("none".to_string(), |(o, a)| format!("+{o:#x} (stand-in {a:#x})"))),
            _ => format!("{c:#x} {} of {:#x}", e.n, e.size),
        })
        .collect();
    match mode {
        GuardMode::Right => hlog!(
            "null guard [{label}]: right: {guarded} guarded stand-ins of {} ({}); {} faults survived; {under} bytes before the stand-ins changed{}",
            STANDINS.count(),
            classes.join(", "),
            GUARD_FAULTS.load(std::sync::atomic::Ordering::Relaxed),
            first_under.map_or(String::new(), |(c, a)| format!(" (first: class {c:#x} stand-in {a:#x})"))
        ),
        _ => hlog!("null guard scan [{label}]: {} stand-ins; per class: {}", STANDINS.count(), classes.join("; ")),
    }
    if STANDINS.dropped() > 0 {
        hlog!("null guard [{label}]: {} stand-ins not recorded (the table is full): not checked", STANDINS.dropped());
    }
}

/// Empty resources not recorded because the table was full.
pub fn standins_dropped() -> usize {
    STANDINS.dropped()
}


// Trampolines: every slot of the null vtable stores its index and the caller's return address, then jumps to
// `null_slot` with the call's own arguments, so the log names the exact exe call site.
std::arch::global_asm!(
    ".globl wh_null_slot_0", "wh_null_slot_0:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 0", "jmp {f}",
    ".globl wh_null_slot_1", "wh_null_slot_1:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 1", "jmp {f}",
    ".globl wh_null_slot_2", "wh_null_slot_2:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 2", "jmp {f}",
    ".globl wh_null_slot_3", "wh_null_slot_3:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 3", "jmp {f}",
    ".globl wh_null_slot_4", "wh_null_slot_4:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 4", "jmp {f}",
    ".globl wh_null_slot_5", "wh_null_slot_5:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 5", "jmp {f}",
    ".globl wh_null_slot_6", "wh_null_slot_6:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 6", "jmp {f}",
    ".globl wh_null_slot_7", "wh_null_slot_7:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 7", "jmp {f}",
    ".globl wh_null_slot_8", "wh_null_slot_8:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 8", "jmp {f}",
    ".globl wh_null_slot_9", "wh_null_slot_9:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 9", "jmp {f}",
    ".globl wh_null_slot_10", "wh_null_slot_10:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 10", "jmp {f}",
    ".globl wh_null_slot_11", "wh_null_slot_11:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 11", "jmp {f}",
    ".globl wh_null_slot_12", "wh_null_slot_12:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 12", "jmp {f}",
    ".globl wh_null_slot_13", "wh_null_slot_13:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 13", "jmp {f}",
    ".globl wh_null_slot_14", "wh_null_slot_14:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 14", "jmp {f}",
    ".globl wh_null_slot_15", "wh_null_slot_15:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 15", "jmp {f}",
    ".globl wh_null_slot_16", "wh_null_slot_16:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 16", "jmp {f}",
    ".globl wh_null_slot_17", "wh_null_slot_17:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 17", "jmp {f}",
    ".globl wh_null_slot_18", "wh_null_slot_18:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 18", "jmp {f}",
    ".globl wh_null_slot_19", "wh_null_slot_19:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 19", "jmp {f}",
    ".globl wh_null_slot_20", "wh_null_slot_20:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 20", "jmp {f}",
    ".globl wh_null_slot_21", "wh_null_slot_21:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 21", "jmp {f}",
    ".globl wh_null_slot_22", "wh_null_slot_22:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 22", "jmp {f}",
    ".globl wh_null_slot_23", "wh_null_slot_23:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 23", "jmp {f}",
    ".globl wh_null_slot_24", "wh_null_slot_24:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 24", "jmp {f}",
    ".globl wh_null_slot_25", "wh_null_slot_25:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 25", "jmp {f}",
    ".globl wh_null_slot_26", "wh_null_slot_26:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 26", "jmp {f}",
    ".globl wh_null_slot_27", "wh_null_slot_27:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 27", "jmp {f}",
    ".globl wh_null_slot_28", "wh_null_slot_28:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 28", "jmp {f}",
    ".globl wh_null_slot_29", "wh_null_slot_29:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 29", "jmp {f}",
    ".globl wh_null_slot_30", "wh_null_slot_30:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 30", "jmp {f}",
    ".globl wh_null_slot_31", "wh_null_slot_31:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 31", "jmp {f}",
    ".globl wh_null_slot_32", "wh_null_slot_32:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 32", "jmp {f}",
    ".globl wh_null_slot_33", "wh_null_slot_33:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 33", "jmp {f}",
    ".globl wh_null_slot_34", "wh_null_slot_34:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 34", "jmp {f}",
    ".globl wh_null_slot_35", "wh_null_slot_35:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 35", "jmp {f}",
    ".globl wh_null_slot_36", "wh_null_slot_36:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 36", "jmp {f}",
    ".globl wh_null_slot_37", "wh_null_slot_37:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 37", "jmp {f}",
    ".globl wh_null_slot_38", "wh_null_slot_38:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 38", "jmp {f}",
    ".globl wh_null_slot_39", "wh_null_slot_39:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 39", "jmp {f}",
    ".globl wh_null_slot_40", "wh_null_slot_40:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 40", "jmp {f}",
    ".globl wh_null_slot_41", "wh_null_slot_41:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 41", "jmp {f}",
    ".globl wh_null_slot_42", "wh_null_slot_42:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 42", "jmp {f}",
    ".globl wh_null_slot_43", "wh_null_slot_43:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 43", "jmp {f}",
    ".globl wh_null_slot_44", "wh_null_slot_44:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 44", "jmp {f}",
    ".globl wh_null_slot_45", "wh_null_slot_45:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 45", "jmp {f}",
    ".globl wh_null_slot_46", "wh_null_slot_46:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 46", "jmp {f}",
    ".globl wh_null_slot_47", "wh_null_slot_47:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 47", "jmp {f}",
    ".globl wh_null_slot_48", "wh_null_slot_48:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 48", "jmp {f}",
    ".globl wh_null_slot_49", "wh_null_slot_49:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 49", "jmp {f}",
    ".globl wh_null_slot_50", "wh_null_slot_50:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 50", "jmp {f}",
    ".globl wh_null_slot_51", "wh_null_slot_51:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 51", "jmp {f}",
    ".globl wh_null_slot_52", "wh_null_slot_52:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 52", "jmp {f}",
    ".globl wh_null_slot_53", "wh_null_slot_53:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 53", "jmp {f}",
    ".globl wh_null_slot_54", "wh_null_slot_54:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 54", "jmp {f}",
    ".globl wh_null_slot_55", "wh_null_slot_55:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 55", "jmp {f}",
    ".globl wh_null_slot_56", "wh_null_slot_56:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 56", "jmp {f}",
    ".globl wh_null_slot_57", "wh_null_slot_57:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 57", "jmp {f}",
    ".globl wh_null_slot_58", "wh_null_slot_58:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 58", "jmp {f}",
    ".globl wh_null_slot_59", "wh_null_slot_59:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 59", "jmp {f}",
    ".globl wh_null_slot_60", "wh_null_slot_60:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 60", "jmp {f}",
    ".globl wh_null_slot_61", "wh_null_slot_61:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 61", "jmp {f}",
    ".globl wh_null_slot_62", "wh_null_slot_62:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 62", "jmp {f}",
    ".globl wh_null_slot_63", "wh_null_slot_63:", "mov rax, [rsp]", "mov [rip + {r}], rax", "mov dword ptr [rip + {n}], 63", "jmp {f}",
    f = sym null_slot,
    r = sym NULL_RET,
    n = sym NULL_SLOT,
);
#[unsafe(no_mangle)]
static mut NULL_RET: u64 = 0;
#[unsafe(no_mangle)]
static mut NULL_SLOT: u32 = 0;
unsafe extern "C" {
    fn wh_null_slot_0();
    fn wh_null_slot_1();
    fn wh_null_slot_2();
    fn wh_null_slot_3();
    fn wh_null_slot_4();
    fn wh_null_slot_5();
    fn wh_null_slot_6();
    fn wh_null_slot_7();
    fn wh_null_slot_8();
    fn wh_null_slot_9();
    fn wh_null_slot_10();
    fn wh_null_slot_11();
    fn wh_null_slot_12();
    fn wh_null_slot_13();
    fn wh_null_slot_14();
    fn wh_null_slot_15();
    fn wh_null_slot_16();
    fn wh_null_slot_17();
    fn wh_null_slot_18();
    fn wh_null_slot_19();
    fn wh_null_slot_20();
    fn wh_null_slot_21();
    fn wh_null_slot_22();
    fn wh_null_slot_23();
    fn wh_null_slot_24();
    fn wh_null_slot_25();
    fn wh_null_slot_26();
    fn wh_null_slot_27();
    fn wh_null_slot_28();
    fn wh_null_slot_29();
    fn wh_null_slot_30();
    fn wh_null_slot_31();
    fn wh_null_slot_32();
    fn wh_null_slot_33();
    fn wh_null_slot_34();
    fn wh_null_slot_35();
    fn wh_null_slot_36();
    fn wh_null_slot_37();
    fn wh_null_slot_38();
    fn wh_null_slot_39();
    fn wh_null_slot_40();
    fn wh_null_slot_41();
    fn wh_null_slot_42();
    fn wh_null_slot_43();
    fn wh_null_slot_44();
    fn wh_null_slot_45();
    fn wh_null_slot_46();
    fn wh_null_slot_47();
    fn wh_null_slot_48();
    fn wh_null_slot_49();
    fn wh_null_slot_50();
    fn wh_null_slot_51();
    fn wh_null_slot_52();
    fn wh_null_slot_53();
    fn wh_null_slot_54();
    fn wh_null_slot_55();
    fn wh_null_slot_56();
    fn wh_null_slot_57();
    fn wh_null_slot_58();
    fn wh_null_slot_59();
    fn wh_null_slot_60();
    fn wh_null_slot_61();
    fn wh_null_slot_62();
    fn wh_null_slot_63();
}
const SLOT_FNS: [unsafe extern "C" fn(); 64] = [
    wh_null_slot_0, wh_null_slot_1, wh_null_slot_2, wh_null_slot_3, wh_null_slot_4, wh_null_slot_5, wh_null_slot_6, wh_null_slot_7, wh_null_slot_8, wh_null_slot_9, wh_null_slot_10, wh_null_slot_11, wh_null_slot_12, wh_null_slot_13, wh_null_slot_14, wh_null_slot_15, wh_null_slot_16, wh_null_slot_17, wh_null_slot_18, wh_null_slot_19, wh_null_slot_20, wh_null_slot_21, wh_null_slot_22, wh_null_slot_23, wh_null_slot_24, wh_null_slot_25, wh_null_slot_26, wh_null_slot_27, wh_null_slot_28, wh_null_slot_29, wh_null_slot_30, wh_null_slot_31, wh_null_slot_32, wh_null_slot_33, wh_null_slot_34, wh_null_slot_35, wh_null_slot_36, wh_null_slot_37, wh_null_slot_38, wh_null_slot_39, wh_null_slot_40, wh_null_slot_41, wh_null_slot_42, wh_null_slot_43, wh_null_slot_44, wh_null_slot_45, wh_null_slot_46, wh_null_slot_47, wh_null_slot_48, wh_null_slot_49, wh_null_slot_50, wh_null_slot_51, wh_null_slot_52, wh_null_slot_53, wh_null_slot_54, wh_null_slot_55, wh_null_slot_56, wh_null_slot_57, wh_null_slot_58, wh_null_slot_59, wh_null_slot_60, wh_null_slot_61, wh_null_slot_62, wh_null_slot_63,
];
unsafe extern "win64" fn null_slot(this: u64, a: u64, b: u64, c: u64) -> u64 {
    // the trampoline left the return address and slot index in two statics (null objects are called from one thread
    // at a time in these runs; good enough for a diagnostic log)
    let (ret, slot) = unsafe { (NULL_RET, NULL_SLOT) };
    hlog!("null object {this:#x}: vtable slot {slot} (+{:#x}) called (args {a:#x}, {b:#x}, {c:#x}) from {ret:#x}", slot * 8);
    0
}

fn res_vtable() -> u64 {
    static VT: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
    *VT.get_or_init(|| unsafe {
        let vt = crate::mem::heap_alloc(8 * SLOT_FNS.len(), true) as u64;
        for (i, f) in SLOT_FNS.iter().enumerate() {
            *((vt + 8 * i as u64) as *mut u64) = *f as *const () as u64;
        }
        vt
    })
}

unsafe fn query_name(q: u64) -> String {
    // the request name pointer lives at query+0x200
    let p = unsafe { *((q + 0x200) as *const u64) };
    if p < 0x10000 || p >= 0x1000_0000_0000 {
        return format!("<name ptr {p:#x}>");
    }
    unsafe { std::ffi::CStr::from_ptr(p as *const std::ffi::c_char) }.to_string_lossy().chars().take(160).collect()
}

unsafe extern "win64" fn null_translate_query(cook: u64, q: u64) -> u64 {
    unsafe {
        let class = *((cook + 0xC) as *const u32);
        let n = {
            let mut c = COUNTS.lock().unwrap();
            let e = c.entry(class).or_insert(0);
            *e += 1;
            *e
        };
        // fresh zero bytes of the class's size (never freed: see +0x1B0)
        let mut size = null_res_size(class);
        let mut guarded = 0;
        let mut r = 0;
        if guard_mode() == GuardMode::Right {
            let g = guard_size(class);
            r = crate::mem::pool_alloc_guarded(g) as u64;
            if r != 0 {
                (size, guarded) = (g, GUARDED_TAG);
            }
        }
        if r == 0 {
            r = crate::mem::pool_alloc(size) as u64;
        }
        if r == 0 {
            hlog!("null cook class {class:#x}: the pool could not map a chunk for {size:#x} bytes: a heap block instead");
            r = crate::mem::heap_alloc(size, true) as u64;
        }
        if r == 0 {
            hlog!("null cook class {class:#x}: out of memory for an empty resource of {size:#x} bytes: abort");
            std::process::abort();
        }
        if crate::pagemap::on() || guard_mode() != GuardMode::Off {
            STANDINS.push(r as usize, size, class | STANDIN_TAG | guarded);
        }
        // the exe's own unmanaged_resource ctor 0x2000B7780(r, 1), as its cooks call it: it sets the kind flags
        // +0x10 = 4 (unmanaged), which the resource registry needs (a zeroed resource faults there)
        crate::run::call2(0x2000B7780, r, 1);
        *(r as *mut u64) = res_vtable();
        // the slot passed to set_result is a resource_ptr that owns one reference (set_result releases it)
        *((r + 0x1B0) as *mut u32) = 2; // + one reference the host keeps: the exe never deletes a null resource (its delete path needs real resource bookkeeping)
        if n <= 3 || n.is_power_of_two() {
            hlog!("null cook class {class:#x}: request '{}' -> empty resource {r:#x} (request {n} of this class)", query_name(q));
        }
        let slot = crate::mem::heap_alloc(8, true) as u64;
        *(slot as *mut u64) = r;
        let desc = crate::mem::heap_alloc(16, true) as u64;
        *(desc as *mut u64) = 0x200AB03B0;
        *((desc + 8) as *mut u32) = NULL_RES_STAT_SIZE;
        crate::run::call3(0x2000BFD10, q, slot, desc);
        crate::run::call4(0x2000C22C0, q, 3, 1, 0);
    }
    0
}

unsafe extern "win64" fn null_delete_resource(cook: u64, res: u64) -> u64 {
    hlog!("null cook {cook:#x}: delete_resource({res:#x}) (kept)");
    0
}

/// Replace the cook of `class` with a null cook. Returns the original cook (0 if none was registered).
pub fn install(class: u32) -> u64 {
    unsafe {
        let table = *(0x200AAF1A0 as *const u64);
        let slot = table + 8 * class as u64;
        let orig = *(slot as *const u64);
        // the cook object: a copy of the original one (same base fields, thread id +0x18) or a fresh one
        let cook = crate::mem::heap_alloc(0x80, true) as u64;
        let vt = crate::mem::heap_alloc(14 * 8, true) as u64;
        if orig != 0 {
            std::ptr::copy_nonoverlapping(orig as *const u8, cook as *mut u8, 0x40);
            std::ptr::copy_nonoverlapping(*(orig as *const u64) as *const u8, vt as *mut u8, 14 * 8);
        } else {
            // fields and base slots of a registered translate_query cook (profile_skin_visual_cook, static object
            // 0x20F4426E0, vtable 0x20095E628), with this class
            std::ptr::copy_nonoverlapping(0x20F4426E0 as *const u8, cook as *mut u8, 0x40);
            std::ptr::copy_nonoverlapping(0x20095E628 as *const u8, vt as *mut u8, 14 * 8);
            *((cook + 0xC) as *mut u32) = class;
        }
        *((vt + 10 * 8) as *mut u64) = null_translate_query as *const () as u64;
        *((vt + 11 * 8) as *mut u64) = null_delete_resource as *const () as u64;
        *(cook as *mut u64) = vt;
        *(slot as *mut u64) = cook;
        if orig != 0 {
            hlog!("null cook installed for class {class:#x} (replaces {orig:#x} {})", crate::run::rtti_name(*(orig as *const u64)));
        }
        orig
    }
}

/// Requests answered by null cooks, per class.
pub fn summary() -> String {
    COUNTS.lock().unwrap().iter().map(|(c, n)| format!("{c:#x}: {n}")).collect::<Vec<_>>().join(", ")
}

/// A null object for a subsystem the world host does not have (e.g. the sound world): a zeroed `size`-byte object
/// whose vtable has 64 logging no-op slots that return 0.
pub fn null_object(size: usize) -> u64 {
    unsafe {
        let o = crate::mem::heap_alloc(size, true) as u64;
        *(o as *mut u64) = res_vtable();
        o
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_slot_covers_the_fields_the_null_path_writes_and_the_real_fixed_size() {
        for class in [0x6, 0xD, 0x13, 0x14, 0x18, 0x3D, 0x79, 0x7B, 0x97, 0x100, 0x101, 0x102, 0x2A] {
            let n = null_res_size(class);
            assert!(n >= 0x230 && n.is_multiple_of(16), "class {class:#x}: {n:#x}");
        }
        // the real fixed sizes (allocation sites in the exe)
        for (class, real) in [(0x6, 0x488), (0xD, 0x358), (0x13, 0x3E8), (0x14, 0x5D58), (0x18, 0x3E8), (0x3D, 0x600), (0x79, 0x350), (0x7B, 0x228), (0x97, 0x238), (0x100, 0x238), (0x101, 0x238), (0x102, 0x240)] {
            assert!(null_res_size(class) >= real, "class {class:#x}");
        }
        // the variable-length classes: their fixed parts, never smaller than the size the guard-page run checked
        assert_eq!([0x79, 0x101, 0x102].map(null_res_size), [0x350, 0x240, 0x240]);
        for class in [0x13, 0x79, 0x101, 0x102] {
            assert!(null_res_size(class) >= guard_default_size(class), "class {class:#x}");
        }
        // 0x13: its fixed part only, the size a guarded run checks; it covers every field the null path writes
        assert_eq!(null_res_size(0x13), 0x3F0);
        assert_eq!(null_res_size(0x13), guard_default_size(0x13));
        assert!(null_res_size(0x13) >= GUARD_MIN);
    }

    #[test]
    fn the_guard_switch_parses_its_mode_and_class_sizes() {
        assert_eq!(parse_guard("right"), Ok((GuardMode::Right, vec![])));
        assert_eq!(parse_guard("scan"), Ok((GuardMode::Scan, vec![])));
        assert_eq!(parse_guard(""), Ok((GuardMode::Off, vec![])));
        assert_eq!(parse_guard("0"), Ok((GuardMode::Off, vec![])));
        assert_eq!(parse_guard("right,0x13=0x1000, 102=1600"), Ok((GuardMode::Right, vec![(0x13, 0x1000), (0x102, 0x1600)])));
        assert_eq!(parse_guard("right,0x13=0x400,0x13=0x3f0"), Ok((GuardMode::Right, vec![(0x13, 0x3F0)])), "the last size of a class counts");
        assert!(parse_guard("left").is_err());
        assert!(parse_guard("scan,0x13=0x1000").is_err(), "sizes only with right");
        assert!(parse_guard("right,0x13").is_err());
        assert!(parse_guard("right,0x13=0x210").is_err(), "smaller than the null path writes");
        assert!(parse_guard("right,0x13=0x3f8").is_err(), "not a multiple of 16");
        assert!(parse_guard("right,x=0x400").is_err());
    }

    #[test]
    fn the_guarded_sizes_never_exceed_the_normal_slots_and_cover_the_null_path() {
        for class in [0x6, 0xD, 0x13, 0x14, 0x18, 0x3D, 0x79, 0x7B, 0x97, 0x100, 0x101, 0x102, 0x2A] {
            let g = guard_default_size(class);
            assert!(g >= GUARD_MIN && g.is_multiple_of(16), "class {class:#x}: {g:#x}");
            assert!(g <= null_res_size(class), "class {class:#x}: a guarded run is never looser than the normal slot");
        }
        // the sizes under test: the fixed parts of the variable-length classes, 0x14 with one page
        assert_eq!([0x13, 0x14, 0x79, 0x101, 0x102].map(guard_default_size), [0x3F0, 0x1000, 0x310, 0x240, 0x240]);
        assert!(guard_default_size(0x13) >= 0x3E8 && guard_default_size(0x102) >= 0x240 && guard_default_size(0x101) >= 0x238);
        // without the switch the sizes are the defaults
        assert_eq!(guard_size(0x6), 0x490);
    }

    #[test]
    fn the_scan_finds_the_last_written_byte() {
        let mut b = vec![0u8; 0x1000];
        assert_eq!(last_nonzero(&b), None);
        b[0] = 1;
        assert_eq!(last_nonzero(&b), Some(0));
        b[0x219] = 1;
        assert_eq!(last_nonzero(&b), Some(0x219));
        b[0xFFF] = 9;
        assert_eq!(last_nonzero(&b), Some(0xFFF));
    }
}
