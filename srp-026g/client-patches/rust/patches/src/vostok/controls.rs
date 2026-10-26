//! The options' writable controls table. Stock rows are read once from the
//! verified image; only our additions are specified here. The game owns
//! key editing, conflict checks, applying and saving every row, including PTT.
use super::Thread;
use crate::raw;

const STOCK_ROWS: usize = 40;
const ROW_WORDS: usize = 26;

struct ExtraControl {
    action: u32,
    label: &'static [u8],
    group: u32,
    kind: u32,
}

// One list for added controls. PTT already has a stock row in the chat group,
// so adding it again would create two competing bindings for the same action.
const EXTRA_CONTROLS: [ExtraControl; 2] = [
    ExtraControl {
        action: 0x19,
        label: b"st_hide_interface_action\0",
        group: 2,
        kind: 1,
    },
    ExtraControl {
        action: 0x37,
        label: b"st_mute_last_speaker_action\0",
        group: 1,
        kind: 0,
    },
];
const ROWS: usize = STOCK_ROWS + EXTRA_CONTROLS.len();

// The game's six readers are pointed here by the build. Only initialize
// writes until the game starts using the rows; afterwards only the game edits
// their strings, on the same main thread. The symbol is consumed by the link.
#[unsafe(no_mangle)]
static mut controls: [u32; ROW_WORDS * ROWS] = [0; ROW_WORDS * ROWS];
static mut INITIALIZED: bool = false;

/// Called once after game_world's constructor and before options and user.cfg.
/// Idempotence matters: a later call must not erase the player's pending keys.
pub fn initialize(_: &Thread) {
    // SAFETY: the game's main thread is proven by Thread. The original table
    // is a verified image global; the destination is our own disjoint storage.
    unsafe {
        if INITIALIZED {
            return;
        }
        populate(
            raw::image::CONTROLS_TABLE as *const u32,
            (&raw mut controls).cast(),
        );
        INITIALIZED = true;
    }
}

/// An exported entry for the emulator's startup check; the actual game calls
/// initialize through the generated, aligned voice registration hook.
///
/// # Safety
/// Must run on the game's main thread with the verified image mapped.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn srp_controls_initialize() {
    // SAFETY: the entry's caller satisfies the same thread contract as a hook.
    initialize(&unsafe { Thread::new() });
}

unsafe fn populate(source: *const u32, destination: *mut u32) {
    for row in 0..ROWS {
        // SAFETY: caller supplies STOCK_ROWS source rows and ROWS destination
        // rows; each row is exactly ROW_WORDS words, verified by selftest.
        unsafe {
            let target = destination.add(row * ROW_WORDS);
            core::ptr::write_bytes(target, 0, ROW_WORDS);
            if row < STOCK_ROWS {
                core::ptr::copy_nonoverlapping(source.add(row * ROW_WORDS), target, 4);
            } else {
                let spec = &EXTRA_CONTROLS[row - STOCK_ROWS];
                *target = spec.action;
                *target.add(1) = spec.label.as_ptr() as u32;
                *target.add(2) = spec.group;
                *target.add(3) = spec.kind;
            }
            // The two buffer_strings each point into their own 32-byte
            // inline storage; no pointer into the source table survives.
            for (header, buffer) in [(4, 7), (15, 18)] {
                *target.add(header) = target.add(buffer) as u32;
                *target.add(header + 1) = target.add(buffer) as u32;
                *target.add(header + 2) = target.add(buffer + 8) as u32;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stock_descriptors_are_preserved_and_each_buffer_is_its_own() {
        let mut stock = [0u32; ROW_WORDS * STOCK_ROWS];
        for row in 0..STOCK_ROWS {
            stock[row * ROW_WORDS..row * ROW_WORDS + 4].copy_from_slice(&[
                row as u32,
                1000 + row as u32,
                1,
                0,
            ]);
        }
        let mut output = [u32::MAX; ROW_WORDS * ROWS];
        // SAFETY: disjoint arrays of the exact required lengths.
        unsafe { populate(stock.as_ptr(), output.as_mut_ptr()) };
        for row in 0..ROWS {
            let at = row * ROW_WORDS;
            if row < STOCK_ROWS {
                assert_eq!(&output[at..at + 4], &stock[at..at + 4]);
            }
            for (header, buffer) in [(4, 7), (15, 18)] {
                let expected = output[at + buffer..].as_ptr() as u32;
                assert_eq!(
                    &output[at + header..at + header + 3],
                    &[expected, expected, expected + 32]
                );
                assert_eq!(&output[at + buffer..at + buffer + 8], &[0; 8]);
            }
        }
        assert_eq!(output[STOCK_ROWS * ROW_WORDS], 0x19);
        assert_eq!(output[(STOCK_ROWS + 1) * ROW_WORDS], 0x37);
    }
}
