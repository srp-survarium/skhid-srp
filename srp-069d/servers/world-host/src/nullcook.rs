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

/// Size of the empty resource object: covers the resource base (refcount +0x1B0, memory stats +0xF0) and the fields
/// the client's users of these classes read first (e.g. the player cook reads +0x210).
const NULL_RES_SIZE: usize = 0x1000;

static COUNTS: Mutex<BTreeMap<u32, u64>> = Mutex::new(BTreeMap::new());


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
        let r = crate::mem::heap_alloc(NULL_RES_SIZE, true) as u64;
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
        *((desc + 8) as *mut u32) = NULL_RES_SIZE as u32;
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
