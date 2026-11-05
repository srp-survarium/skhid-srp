//! Breadcrumbs for a crash report: the last [`CAPACITY`] notable events of
//! the game (a match's options, a world loaded, a death, a pickup...) and
//! the current match's context, kept in the arena's `patches.state` (zeros
//! at load, never allocated) and read by the crash reporter after a crash
//! (`client/bugtrap/src/breadcrumbs.rs`, which knows the layout below).
//!
//! Only notable events record one: never a frame or a tick. A record is a
//! QueryPerformanceCounter read and a dozen stores; nothing here changes
//! what a match computes (the ring is written, never read, by the game).
//!
//! The reporter finds the ring by its header: the header's own address,
//! then that address xor [`CHECK`] (a pair no constant in the image can be,
//! as it names its own place), written at the first record.

use core::cell::UnsafeCell;

use super::clock;
use super::hook::Thread;

/// Entries kept: the last 1024 events (32 KiB).
pub const CAPACITY: usize = 1024;
/// The header's second word: its address xor this ("BRCS").
pub const CHECK: u32 = u32::from_le_bytes(*b"BRCS");
/// The layout's version (the reporter's `breadcrumbs.rs` reads version 1).
const VERSION: u16 = 1;

/// A notable event, with what is known of it then.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Crumb {
    /// A match's options read: the static info's (a live match) or a
    /// replay's.
    Match { replay: bool, context: Match },
    /// A world (0xE8) about to be loaded: its game time, bytes, whether it
    /// came packed, and the local place then; `loaded` false when patch 87
    /// refused it.
    World { time: u32, bytes: u32, packed: bool, local: Option<u8>, loaded: bool },
    /// The countdown ended (the welcome message): the match is played.
    Started,
    /// The game unloads its menus and world (game::clear_resources).
    Unloaded,
    /// A player is being made (its cook's batch in: a kit, a respawn, a late
    /// joiner).
    PlayerMade,
    /// A place's player is destroyed (it left, or the match's teardown).
    PlayerGone { place: u8 },
    /// A place's player died: its life and the game time.
    Death { place: u8, life: u32, time: u32 },
    /// Patch 85: a picked gun's step at a place's weapon slot.
    Pickup { event: Pickup, place: u8, slot: u8, from: u8, kit: u8, time: u32 },
}

/// A match's options: what names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Match {
    /// The match id the server sends (its seed).
    pub id: u32,
    /// The maps_dict id (the map and its mode).
    pub map: u8,
    pub places: u8,
    pub level: u16,
    pub blackout: bool,
}

/// A step of patch 85's pickup (weapon_pickup.rs's log lines).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Pickup {
    Switched = 1,
    LetGo = 2,
    BackToDrop = 3,
    RestoreFailed = 4,
    Cancelled = 5,
    Replaced = 6,
    Loaded = 7,
    LoadedNotInstalled = 8,
    PendingNotRegistered = 9,
    Ignored = 10,
    Waiting = 11,
}

/// An entry as the reporter reads it: 32 bytes.
#[derive(Clone, Copy)]
#[repr(C)]
struct Entry {
    /// The record's number + 1, written last (0: never written).
    seq: u32,
    kind: u8,
    a: u8,
    b: u16,
    /// QueryPerformanceCounter at the record.
    counter: u64,
    values: [u32; 4],
}

const _: () = assert!(size_of::<Entry>() == 32);

/// The header: 64 bytes, before the entries.
#[derive(Clone, Copy)]
#[repr(C)]
struct Header {
    /// The header's own address, then it xor [`CHECK`] (0 until the first
    /// record).
    at: u32,
    check: u32,
    version: u16,
    entry_size: u16,
    capacity: u32,
    /// Records made; the next goes to `next % CAPACITY`.
    next: u32,
    _pad: u32,
    /// QueryPerformanceCounter's ticks a second.
    frequency: u64,
    /// The last match's options ([`Match`]): `flags` bit 0 Blackout, bit 1
    /// a replay, bit 31 set once there is one.
    match_id: u32,
    map: u8,
    places: u8,
    level: u16,
    flags: u32,
    /// The local place at the last world load + 1 (0: none known).
    local: u32,
    _reserved: [u32; 4],
}

const _: () = assert!(size_of::<Header>() == 64);

#[repr(C, align(64))]
struct Ring {
    header: Header,
    entries: [Entry; CAPACITY],
}

const ZERO_ENTRY: Entry = Entry { seq: 0, kind: 0, a: 0, b: 0, counter: 0, values: [0; 4] };

struct Cell(UnsafeCell<Ring>);

// SAFETY: written only through `record`, which takes the game's Thread (the
// hooks that record run on the game's thread); the reporter reads the
// memory from outside Rust after a crash.
unsafe impl Sync for Cell {}

/// Zeros: the arena's `patches.state`.
static RING: Cell = Cell(UnsafeCell::new(Ring {
    header: Header {
        at: 0,
        check: 0,
        version: 0,
        entry_size: 0,
        capacity: 0,
        next: 0,
        _pad: 0,
        frequency: 0,
        match_id: 0,
        map: 0,
        places: 0,
        level: 0,
        flags: 0,
        local: 0,
        _reserved: [0; 4],
    },
    entries: [ZERO_ENTRY; CAPACITY],
}));

/// The entry's kind, its two small values and its four words.
fn encode(crumb: Crumb) -> (u8, u8, u16, [u32; 4]) {
    match crumb {
        Crumb::Match { replay, context: m } => {
            (1, u8::from(replay), m.level, [m.id, u32::from(m.map), u32::from(m.places), u32::from(m.blackout)])
        }
        Crumb::World { time, bytes, packed, local, loaded } => {
            (2, u8::from(packed) | u8::from(loaded) << 1, local.map_or(0, |p| u16::from(p) + 1), [time, bytes, 0, 0])
        }
        Crumb::Started => (3, 0, 0, [0; 4]),
        Crumb::Unloaded => (4, 0, 0, [0; 4]),
        Crumb::PlayerMade => (5, 0, 0, [0; 4]),
        Crumb::PlayerGone { place } => (6, place, 0, [0; 4]),
        Crumb::Death { place, life, time } => (7, place, 0, [life, time, 0, 0]),
        Crumb::Pickup { event, place, slot, from, kit, time } => {
            (8, event as u8, u16::from(place) | u16::from(slot) << 8, [u32::from(from), u32::from(kit), time, 0])
        }
    }
}

/// Records `crumb`, and keeps the match's context in the header.
pub fn record(thread: &Thread, crumb: Crumb) {
    write(thread, clock::counter(thread), || clock::frequency(thread), crumb);
}

/// [`record`] at the counter `counter` (`frequency` asked at the first).
fn write(_: &Thread, counter: u64, frequency: impl FnOnce() -> u64, crumb: Crumb) {
    let (kind, a, b, values) = encode(crumb);
    let ring = RING.0.get();
    // SAFETY: the ring is a static only this function writes, on the game's
    // thread (the Thread); volatile, as the reporter reads it from outside.
    unsafe {
        let header = &raw mut (*ring).header;
        if (*header).at == 0 {
            let at = header as u32;
            (*header).version = VERSION;
            (*header).entry_size = size_of::<Entry>() as u16;
            (*header).capacity = CAPACITY as u32;
            (*header).frequency = frequency();
            (&raw mut (*header).check).write_volatile(at ^ CHECK);
            (&raw mut (*header).at).write_volatile(at);
        }
        match crumb {
            Crumb::Match { replay, context: m } => {
                (*header).match_id = m.id;
                (*header).map = m.map;
                (*header).places = m.places;
                (*header).level = m.level;
                (*header).local = 0;
                (&raw mut (*header).flags).write_volatile(u32::from(m.blackout) | u32::from(replay) << 1 | 1 << 31);
            }
            Crumb::World { local, .. } => {
                (&raw mut (*header).local).write_volatile(local.map_or(0, |p| u32::from(p) + 1));
            }
            _ => {}
        }
        let n = (*header).next;
        let entry = &raw mut (*ring).entries[n as usize % CAPACITY];
        // Never a half-written entry with a valid number: 0 first, the number last.
        (&raw mut (*entry).seq).write_volatile(0);
        entry.write_volatile(Entry { seq: 0, kind, a, b, counter, values });
        (&raw mut (*entry).seq).write_volatile(n.wrapping_add(1).max(1));
        (&raw mut (*header).next).write_volatile(n.wrapping_add(1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_crumb_is_its_kind_and_values() {
        let m = Match { id: 77, map: 12, places: 8, level: 5, blackout: true };
        assert_eq!(encode(Crumb::Match { replay: false, context: m }), (1, 0, 5, [77, 12, 8, 1]));
        assert_eq!(
            encode(Crumb::World { time: 4500, bytes: 900, packed: true, local: Some(3), loaded: true }),
            (2, 3, 4, [4500, 900, 0, 0])
        );
        assert_eq!(encode(Crumb::World { time: 1, bytes: 2, packed: false, local: None, loaded: false }).1, 0);
        assert_eq!(encode(Crumb::Death { place: 4, life: 2, time: 9000 }), (7, 4, 0, [2, 9000, 0, 0]));
        assert_eq!(
            encode(Crumb::Pickup { event: Pickup::LetGo, place: 1, slot: 2, from: 5, kit: 3, time: 10 }),
            (8, 2, 0x0201, [5, 3, 10, 0])
        );
    }

    #[test]
    fn the_ring_keeps_the_last_entries_and_the_match() {
        // SAFETY: the test is the only thread.
        let thread = unsafe { Thread::new() };
        let m = Match { id: 9, map: 3, places: 2, level: 1, blackout: false };
        write(&thread, 5, || 1000, Crumb::Match { replay: true, context: m });
        for place in 0..(CAPACITY as u32 + 5) {
            write(&thread, 5, || 1000, Crumb::PlayerGone { place: place as u8 });
        }
        write(&thread, 5, || 1000, Crumb::World { time: 1, bytes: 2, packed: false, local: Some(6), loaded: true });
        // SAFETY: no record runs meanwhile.
        let ring = unsafe { &*RING.0.get() };
        let h = &ring.header;
        assert_eq!((h.at, h.check ^ h.at, h.capacity, h.entry_size, h.version), (h.at, CHECK, 1024, 32, 1));
        assert_eq!(h.at, &raw const ring.header as u32);
        assert_eq!((h.match_id, h.map, h.places, h.flags, h.local), (9, 3, 2, 1 << 31 | 2, 7));
        let n = h.next as usize;
        assert_eq!(n, CAPACITY + 7);
        let last = &ring.entries[(n - 1) % CAPACITY];
        assert_eq!((last.seq as usize, last.kind, last.b), (n, 2, 7));
        // The match's entry is gone from the ring; the header still has it.
        assert!(ring.entries.iter().all(|e| e.kind != 1));
    }
}
