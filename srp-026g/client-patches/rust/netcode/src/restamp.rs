//! The tick's sites in the exe, stamped for the match's grid (phase 4,
//! `netcode_port_069.md` 6.1; the table: `sites.rs`, made by
//! `tick_sites_026g.py`). The exe is built at 50 ms: a match of 50 ms
//! (every match unless the server's `SRP_NETCODE_TICK_MS` says otherwise)
//! changes no byte. Every static info stamps the match's tick (a stock one
//! 50), and so does a replay's start (50: the game's replays are of 50 ms).
//!
//! `.text` is read-only at run time: each write is made writable with
//! VirtualProtect (the exe's own import) and put back. Before a stamp,
//! every site must hold what the last stamp wrote: a byte anyone else
//! changed leaves the exe as it is (and a line in the log), never half
//! stamped.

use sync_core::TickLen;

/// What a site is (`tick_sites_026g.py`'s kinds).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// The imm32 of a division by the tick.
    Magic,
    /// Its `shr`'s count.
    Shift,
    /// An imm8 of the tick.
    Tick8,
    /// A disp8 of minus the tick.
    Back8,
    /// An imm32 of the tick.
    Tick32,
    /// The hash interval (patch 3).
    Hash8,
    /// The disp32 to the float 1/tick of the interpolated view.
    Fraction,
    /// The disp32 to the physics' fixed step.
    Physics,
}

/// A byte (or four) the tick is in.
#[derive(Clone, Copy, Debug)]
pub struct Site {
    pub at: u32,
    pub kind: Kind,
}

/// The interpolated view's 1/T (s): 0.02 at 50 is the game's own
/// (0xA2DB34, also a hand offset: never written).
static FRACTION_25: f32 = 0.04;
static FRACTION_20: f32 = 0.05;
/// The physics' substep: a third of the tick, as the game's 1/60 is of 50
/// ms (0xA2DBB4, the weapon cook's too: never written).
static PHYSICS_25: f32 = 0.025 / 3.0;
static PHYSICS_20: f32 = 0.020 / 3.0;
const STOCK_FRACTION: u32 = 0xA2DB34;
const STOCK_PHYSICS: u32 = 0xA2DBB4;

/// A site's value on `tick`, and its width.
fn value(kind: Kind, tick: TickLen) -> (u32, usize) {
    let ms = tick.ms();
    let address = |value: &'static f32| value as *const f32 as usize as u32;
    match kind {
        Kind::Magic => (if tick == TickLen::Ms20 { 0xCCCC_CCCD } else { 0x51EB_851F }, 4),
        Kind::Shift => (if tick == TickLen::Ms25 { 3 } else { 4 }, 1),
        Kind::Tick8 => (ms, 1),
        Kind::Back8 => (ms.wrapping_neg() & 0xFF, 1),
        Kind::Tick32 => (ms, 4),
        Kind::Hash8 => (50 / ms * ms, 1),
        Kind::Fraction => match tick {
            TickLen::Ms50 => (STOCK_FRACTION, 4),
            TickLen::Ms25 => (address(&FRACTION_25), 4),
            TickLen::Ms20 => (address(&FRACTION_20), 4),
        },
        Kind::Physics => match tick {
            TickLen::Ms50 => (STOCK_PHYSICS, 4),
            TickLen::Ms25 => (address(&PHYSICS_25), 4),
            TickLen::Ms20 => (address(&PHYSICS_20), 4),
        },
    }
}

/// The value bytes of a site.
fn bytes(kind: Kind, tick: TickLen) -> ([u8; 4], usize) {
    let (value, width) = value(kind, tick);
    (value.to_le_bytes(), width)
}

/// Why a stamp was not made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    /// A site holds what no stamp wrote.
    Changed { at: u32 },
    /// VirtualProtect refused a site.
    Protect { at: u32 },
}

/// The exe's memory, as the stamp reads and writes it (the game's, or a
/// test's).
pub trait Code {
    fn read(&self, at: u32, out: &mut [u8]);
    /// Writes `bytes` at `at`, made writable for it: false if it could not.
    fn write(&mut self, at: u32, bytes: &[u8]) -> bool;
}

/// Stamps `sites` from `from` (what they hold) to `to`: all of them, or
/// none if one holds anything else.
pub fn stamp(code: &mut impl Code, sites: &[Site], from: TickLen, to: TickLen) -> Result<(), Refused> {
    for site in sites {
        let (want, width) = bytes(site.kind, from);
        let mut have = [0u8; 4];
        code.read(site.at, &mut have[..width]);
        // Byte by byte: a slice compare is a call of memcmp, which the
        // arena has not.
        if (0..width).any(|i| have[i] != want[i]) {
            return Err(Refused::Changed { at: site.at });
        }
    }
    if from == to {
        return Ok(());
    }
    for site in sites {
        let (new, width) = bytes(site.kind, to);
        if !code.write(site.at, &new[..width]) {
            return Err(Refused::Protect { at: site.at });
        }
    }
    Ok(())
}

#[cfg(target_arch = "x86")]
mod game {
    use core::ffi::c_void;

    use patch_core::log::{Piece::*, line};
    use patch_core::{Import, Va};
    use sync_core::{Grid, TickLen};

    use super::{Code, Refused, stamp};
    use crate::sites::SITES;

    /// What the exe's sites hold: the build's 50 until a stamp.
    static mut STAMPED: TickLen = TickLen::Ms50;

    /// kernel32's VirtualProtect, by the exe's IAT slot (0x870200).
    const VIRTUAL_PROTECT: Import<unsafe extern "stdcall" fn(*mut c_void, usize, u32, *mut u32) -> i32> =
        Import::at(Va(0x870200));
    const PAGE_EXECUTE_READWRITE: u32 = 0x40;

    struct Exe;

    impl Code for Exe {
        fn read(&self, at: u32, out: &mut [u8]) {
            for (i, byte) in out.iter_mut().enumerate() {
                // SAFETY: a site of the exe's code, mapped.
                *byte = unsafe { core::ptr::read_volatile((at as usize + i) as *const u8) };
            }
        }

        fn write(&mut self, at: u32, bytes: &[u8]) -> bool {
            let place = at as usize as *mut c_void;
            let mut old = 0u32;
            // SAFETY: the exe's code; made writable for the write, put back.
            unsafe {
                if VIRTUAL_PROTECT.get()(place, bytes.len(), PAGE_EXECUTE_READWRITE, &mut old) == 0 {
                    return false;
                }
                for (i, byte) in bytes.iter().enumerate() {
                    core::ptr::write_volatile((at as usize + i) as *mut u8, *byte);
                }
                let mut again = 0u32;
                VIRTUAL_PROTECT.get()(place, bytes.len(), old, &mut again);
            }
            true
        }
    }

    /// Stamps the exe for `grid`: whether it is on it.
    pub fn to(grid: Grid) -> bool {
        let tick = grid.tick();
        // SAFETY: the logic thread alone stamps (a static info, a replay's
        // start), before the match's world runs.
        let from = unsafe { *(&raw const STAMPED) };
        match stamp(&mut Exe, &SITES, from, tick) {
            Ok(()) => {
                if from != tick {
                    unsafe { *(&raw mut STAMPED) = tick };
                    line(&[Text("netcode: the tick is "), Dec(tick.ms()), Text(" ms")]);
                }
                true
            }
            Err(Refused::Changed { at }) => {
                line(&[Text("netcode: tick site "), Hex(at), Text(" holds what no stamp wrote; the tick stays "),
                       Dec(from.ms()), Text(" ms")]);
                false
            }
            Err(Refused::Protect { at }) => {
                // Some sites are on the new tick: the match cannot be
                // played right either way; the log says so.
                unsafe { *(&raw mut STAMPED) = tick };
                line(&[Text("netcode: VirtualProtect refused tick site "), Hex(at)]);
                false
            }
        }
    }
}

#[cfg(target_arch = "x86")]
pub use game::to;

/// Host builds: nothing to stamp.
#[cfg(not(target_arch = "x86"))]
pub fn to(grid: sync_core::Grid) -> bool {
    let _ = grid;
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sites::SITES;
    use std::collections::BTreeMap;
    use std::vec::Vec;

    /// The sites' bytes as the original exe has them (patch 3's hash 50).
    struct Memory(BTreeMap<u32, u8>);

    impl Memory {
        fn built() -> Memory {
            let mut memory = BTreeMap::new();
            for site in SITES {
                let (bytes, width) = bytes(site.kind, TickLen::Ms50);
                for (i, byte) in bytes[..width].iter().enumerate() {
                    memory.insert(site.at + i as u32, *byte);
                }
            }
            Memory(memory)
        }
    }

    impl Code for Memory {
        fn read(&self, at: u32, out: &mut [u8]) {
            for (i, byte) in out.iter_mut().enumerate() {
                *byte = self.0[&(at + i as u32)];
            }
        }

        fn write(&mut self, at: u32, bytes: &[u8]) -> bool {
            for (i, byte) in bytes.iter().enumerate() {
                self.0.insert(at + i as u32, *byte);
            }
            true
        }
    }

    #[test]
    fn the_divisions_are_exact_on_every_tick() {
        for tick in [TickLen::Ms20, TickLen::Ms25, TickLen::Ms50] {
            let magic = u64::from(value(Kind::Magic, tick).0);
            let shift = 32 + value(Kind::Shift, tick).0;
            for x in (0..u32::MAX).step_by(9_973).chain([u32::MAX, u32::MAX - 1, 49, 50, 99, 100]) {
                assert_eq!((u64::from(x) * magic) >> shift, u64::from(x / tick.ms()), "{x} / {}", tick.ms());
            }
        }
    }

    #[test]
    fn a_stamp_goes_there_and_back() {
        let mut memory = Memory::built();
        let built = memory.0.clone();
        assert_eq!(stamp(&mut memory, &SITES, TickLen::Ms50, TickLen::Ms50), Ok(()));
        assert_eq!(memory.0, built);
        assert_eq!(stamp(&mut memory, &SITES, TickLen::Ms50, TickLen::Ms25), Ok(()));
        let changed: Vec<u32> = memory.0.iter().filter(|(at, byte)| built[at] != **byte).map(|(at, _)| *at).collect();
        assert!(changed.len() > 100);
        assert_eq!(stamp(&mut memory, &SITES, TickLen::Ms25, TickLen::Ms20), Ok(()));
        assert_eq!(stamp(&mut memory, &SITES, TickLen::Ms20, TickLen::Ms50), Ok(()));
        assert_eq!(memory.0, built);
    }

    #[test]
    fn a_site_someone_changed_leaves_every_site() {
        let mut memory = Memory::built();
        let at = SITES[10].at;
        memory.0.insert(at, 0x90);
        let before = memory.0.clone();
        assert_eq!(
            stamp(&mut memory, &SITES, TickLen::Ms50, TickLen::Ms25),
            Err(Refused::Changed { at })
        );
        assert_eq!(memory.0, before);
    }

    #[test]
    fn the_values_of_a_tick() {
        assert_eq!(value(Kind::Back8, TickLen::Ms25).0, 0xE7);
        assert_eq!(value(Kind::Back8, TickLen::Ms50).0, 0xCE);
        assert_eq!(value(Kind::Hash8, TickLen::Ms20).0, 40);
        assert_eq!(value(Kind::Hash8, TickLen::Ms25).0, 50);
        assert_eq!(value(Kind::Hash8, TickLen::Ms50).0, 50);
        assert_eq!(FRACTION_25, 0.04);
    }
}
