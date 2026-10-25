//! The late join's code after late_join_026g.asm in `.srp` (patch 11): its
//! entries, which the assembly calls (cdecl; late_join_026g.py gives it
//! their addresses), the crate late-join on the game's memory, and the
//! native install (build 2777's slot made for the newcomer).
//!
//! Linked at 0 and loaded where the assembly ends: no static storage, no
//! constant data, nothing but calls relative to itself (late_join_026g.py
//! refuses anything else). The game's frames are aligned to 4 and Rust's
//! i686 code assumes 16 at a call, so every entry is a naked stub that
//! aligns the stack and calls the function after it ([`entry`]).

#![no_std]

use core::ffi::c_void;
use core::ptr::null_mut;
use core::sync::atomic::{AtomicU32, Ordering};

use late_join::runtime::{self, Context, State};
use late_join::validate::{self, Dictionary, ItemRecord, Modification};

// arena_rt: the panic handler (and memcpy, memset if LLVM calls them).
use arena_rt as _;

/// `$name`, a cdecl entry of `$offsets` (the arguments' offsets from EBP
/// after `push ebp`, the last first), which calls `$inner` with the same
/// arguments on a stack aligned to 16; EAX is `$inner`'s.
macro_rules! entry {
    ($(#[$meta:meta])* $name:ident => $inner:path, [$($at:literal),*]) => {
        $(#[$meta])*
        #[unsafe(naked)]
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name() {
            core::arch::naked_asm!(
                "push ebp",
                "mov ebp, esp",
                "and esp, -16",
                "sub esp, {pad}",
                $(concat!("push dword ptr [ebp + ", $at, "]"),)*
                "call {inner}",
                "leave",
                "ret",
                pad = const (16 - 4 * [$($at),*].len() % 16) % 16,
                inner = sym $inner,
            )
        }
    };
}

entry!(
    /// cdecl(bytes, length, dictionary) -> bool, the assembly's C_CODE:
    /// first in the code.
    #[unsafe(link_section = ".text.entry")]
    late_join_validate_profile => validate_profile, [16, 12, 8]
);
entry!(lj_activate => activate, [28, 24, 20, 16, 12, 8]);
entry!(lj_deactivate => deactivate, [8]);
entry!(lj_prepare => prepare, [16, 12, 8]);
entry!(lj_complete => complete, [12, 8]);
entry!(lj_player_destroyed => player_destroyed, [16, 12, 8]);
entry!(lj_native_install => native_install, [12, 8]);
entry!(lj_native_animations => native_animations, [16, 12, 8]);

/// A pointer of the game's at `base + offset`.
unsafe fn ptr(base: *const u8, offset: usize) -> *mut u8 {
    unsafe { base.add(offset).cast::<*mut u8>().read_unaligned() }
}

unsafe fn word(base: *const u8, offset: usize) -> u32 {
    unsafe { base.add(offset).cast::<u32>().read_unaligned() }
}

unsafe fn set<T>(base: *mut u8, offset: usize, value: T) {
    unsafe { base.add(offset).cast::<T>().write_unaligned(value) }
}

/// The game's live items_dictionary.
struct Items(*const u8);

impl Dictionary for Items {
    fn item(&self, id: u16) -> Option<ItemRecord> {
        unsafe {
            let rows = ptr(self.0, 0x130);
            let count = word(self.0, 0x134) as usize;
            (0..count).map(|i| rows.add(i * 0x3A0)).find(|&row| row.cast::<u16>().read_unaligned() == id).map(|row| {
                if *row.add(0x22A) != 0 { ItemRecord::Short } else { ItemRecord::Long }
            })
        }
    }

    fn has_modification(&self, id: u32, kind: Modification) -> bool {
        let table = match kind {
            Modification::Defect => 0x148,
            Modification::Ordinary => 0x140,
        };
        unsafe {
            let rows = ptr(self.0, table);
            let count = word(self.0, table + 4) as usize;
            (0..count).any(|i| word(rows, i * 0x2C) == id)
        }
    }
}

extern "C" fn validate_profile(bytes: *const u8, length: u32, dictionary: *const u8) -> u32 {
    if bytes.is_null() || dictionary.is_null() || length as usize > validate::MAX_BYTES {
        return 0;
    }
    let bytes = unsafe { core::slice::from_raw_parts(bytes, length as usize) };
    u32::from(validate::profile(bytes, &Items(dictionary)))
}

unsafe extern "C" fn activate(
    s: *mut State,
    client: *mut c_void,
    world: *mut c_void,
    dictionary: *mut c_void,
    seed: u32,
    roster: u32,
) {
    unsafe { runtime::activate(s, client, world, dictionary, seed, roster) }
}

unsafe extern "C" fn deactivate(s: *mut State) {
    unsafe { runtime::deactivate(s) }
}

unsafe extern "C" fn prepare(s: *mut State, packet: *const u8, length: u32) {
    if !packet.is_null() {
        unsafe { runtime::prepare(s, core::slice::from_raw_parts(packet, length as usize)) }
    }
}

unsafe extern "C" fn complete(c: *mut Context, players: *const [*mut c_void; 3]) {
    unsafe { runtime::complete(c, &*players) }
}

unsafe extern "C" fn player_destroyed(s: *mut State, player: *mut c_void, profile: *const c_void) {
    unsafe { runtime::player_destroyed(s, player, profile) }
}

/// The game's functions the install calls (late_join_026g.asm's adapters, cdecl).
#[repr(C)]
struct Bindings {
    core: unsafe extern "C" fn(core: *mut u8, slot: *mut u8),
    physics: unsafe extern "C" fn(physics: *mut u8, slot: *mut u8),
    animations: unsafe extern "C" fn(player: *mut u8, callbacks: *mut u8),
    ready: unsafe extern "C" fn(player: *mut u8),
    slot_callback: *mut c_void,
}

/// Finalize each kit in roster order; later kits append without renumbering.
unsafe extern "C" fn native_animations(
    table: *mut u8,
    append: unsafe extern "C" fn(*mut u8, *mut u8),
    less: unsafe extern "C" fn(*mut u8, *mut u8) -> i32,
) {
    unsafe {
        let vector = table.add(0xE044);
        let head = table.add(0xE034);
        let begin = ptr(vector, 0).cast::<*mut u8>();
        let first = (ptr(vector, 4) as usize).wrapping_sub(ptr(vector, 0) as usize) as u32 / 4;
        let mut node = ptr(head, 4);
        while node != head {
            let entry = node.sub(12);
            let id = word(entry, 8);
            if id >= first || *begin.add(id as usize) != entry {
                append(vector, entry);
            }
            if !ptr(node, 8).is_null() {
                node = ptr(node, 8);
                while !ptr(node, 4).is_null() {
                    node = ptr(node, 4);
                }
            } else {
                let mut parent = ptr(node, 0);
                while node == ptr(parent, 8) {
                    node = parent;
                    parent = ptr(parent, 0);
                }
                if ptr(node, 8) != parent {
                    node = parent;
                }
            }
        }
        let count = (ptr(vector, 4) as usize).wrapping_sub(ptr(vector, 0) as usize) as u32 / 4;
        let mut i = first.wrapping_add(1);
        while i < count {
            let entry = *begin.add(i as usize);
            let mut j = i;
            while j > first && less(entry, *begin.add(j as usize - 1)) != 0 {
                *begin.add(j as usize) = *begin.add(j as usize - 1);
                j -= 1;
            }
            *begin.add(j as usize) = entry;
            i += 1;
        }
        let mut i = first;
        while i < count {
            set(*begin.add(i as usize), 8, i);
            i += 1;
        }
        *table.add(0x10050) = 1;
    }
}

/// The slot just bound put `added` nodes at the front of the list the world
/// saves its game objects by (core+0x18914; bind_slot_reserved in the
/// assembly). The states the game keeps to roll back to (core+0x2FBC the
/// oldest, item+0xC0BAC the next) were saved without them: their
/// game-objects bytes (the buffer at item+0xBDF30: +4 the bytes, +8 the room,
/// +12 the size; made by 0x3246D0 with room for 0x2000 and nothing to grow
/// by) get the new nodes' bytes in front, so a state loaded later reads in
/// step. Such a node is a grenade or a trap that nobody has touched, and
/// each saves itself as one zero byte (grenade_core::serialize 0x303DA0,
/// booby_trap_core::serialize 0x3052E0). A state with no room for them is
/// left as it is: the largest seen is 525 bytes of the 8192.
unsafe fn shift_saved_objects(core: *mut u8, added: u32) {
    unsafe {
        let mut item = ptr(core, 0x2FBC);
        while !item.is_null() {
            let buffer = item.add(0xBDF30);
            let bytes = ptr(buffer, 4);
            let size = word(buffer, 12);
            let room = word(buffer, 8);
            if !bytes.is_null() && size <= room && added <= room - size {
                let mut i = size;
                while i != 0 {
                    *bytes.add((i - 1 + added) as usize) = *bytes.add(i as usize - 1);
                    i -= 1;
                }
                for i in 0..added as usize {
                    *bytes.add(i) = 0;
                }
                set(buffer, 12, size + added);
            }
            item = ptr(item, 0xC0BAC);
        }
    }
}

/// Build-2777 native slot installation. Runs on the game's resource-callback
/// thread, between simulation updates. No player-entry event is emitted here.
unsafe extern "C" fn native_install(c: *mut Context, bind: *const Bindings) -> u32 {
    unsafe {
        let c = &*c;
        let bind = &*bind;
        let nc = c.client.cast::<u8>();
        let pvp = c.world.cast::<u8>();
        let core = pvp.add(0x1F8);
        if c.kit_mask == 0 || c.kit_mask & !7 != 0 {
            return 0;
        }
        for (kit, player) in c.players.iter().enumerate() {
            if !player.is_null() != (c.kit_mask & 1 << kit != 0) {
                return 0;
            }
        }
        if c.place >= 20 || word(nc, 0x1F1C64) != 2 || ptr(nc, 0x1F1C40) != pvp {
            return 0;
        }
        let options = ptr(pvp, 0x124);
        let begin = ptr(core, 0x18458);
        let end = ptr(core, 0x1845C);
        let capacity = ptr(core, 0x18460);
        let statistics = ptr(core, 0x18FA8);
        if options.is_null()
            || begin.is_null()
            || statistics.is_null()
            || u32::from(*options.add(0x577B5)) != c.place
            || end != begin.wrapping_add(48 * c.place as usize)
            || (capacity as u32) < (end as u32).wrapping_add(48)
            || word(options, 0x577B0) != c.seed
        {
            return 0;
        }
        let profile = options.add(c.place as usize * 0x45FC);
        let s = &*c.state;
        let Some(decode) = s.api.decode else { return 0 };
        if decode(c.wire(), c.wire_size, profile.cast(), s.dictionary) == 0 {
            return 0;
        }

        // Every check that can refuse the proposal precedes the slot's
        // publication. Each query result already has a reference while its
        // callback runs.
        for i in 0..48 {
            *end.add(i) = 0;
        }
        *end.add(12) = 0xFF;
        *end.add(13) = 0xFF;
        *end.add(0x24) = 0xFF;
        set(end, 0x10, end);
        set(end, 0x14, bind.slot_callback);
        set(end, 0x20, 0xFFFF_FFFFu32);
        *end.add(0x2C) = c.place as u8;
        for (kit, &player) in c.players.iter().enumerate() {
            let player = player.cast::<u8>();
            if player.is_null() {
                continue;
            }
            AtomicU32::from_ptr(player.add(0xD0).cast()).fetch_add(1, Ordering::Relaxed);
            set(end, kit * 4, player);
            // 0x2CBC60 passes this animation table to 0x2D8100; the PVP
            // constructor retains it at +0x19238. Spatial tree is +0x130.
            (bind.animations)(player, ptr(pvp, 0x19238));
        }
        set(core, 0x1845C, end.add(48));
        *options.add(0x5782C + c.place as usize) = c.squad as u8;
        *options.add(0x577B5) = (c.place + 1) as u8;
        // core stores statistics_handler; its shared_statistics subobject is
        // +8 (native serialize thunk 0x26DB40), with the live count at +0x31E0.
        *statistics.add(8 + 0x31E0) = (c.place + 1) as u8;
        let listed = ptr(core, 0x18914);
        (bind.core)(core, end);
        let mut added = 0u32;
        let mut node = ptr(core, 0x18914);
        while !node.is_null() && node != listed && added < 0x2000 {
            added += 1;
            node = ptr(node, 8);
        }
        // More nodes than a state has room for is no list of a slot's.
        if added != 0 && added < 0x2000 {
            shift_saved_objects(core, added);
        }
        (bind.physics)(pvp.add(0x138), end);
        (bind.ready)(ptr(end, usize::from(*end.add(12)) * 4));
        1
    }
}

const _: fn() = || {
    let _ = null_mut::<u8>;
};
