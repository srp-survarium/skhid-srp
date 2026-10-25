//! Patch 74: the client side of the sync core (`docs/notes/sync_core.md`).
//!
//! `sync_core_026g.asm`'s `sc_render_move` stands in for game_world::tick's
//! call of `game_world_core::move` (0x1F7C37) and calls
//! [`sync_render_move`]; the decision is `sync_core::pacing`'s, the move
//! the game's own through the engine library.

#![no_std]

use core::ffi::c_void;
use core::ptr::{read_volatile, write_volatile};

use sync_core::pacing::{self, Pacing, Plan, WorldView};

mod engine {
    include!("../../engine_026g.rs");
}

// game_world_core's fields (sync_core.md 1.1).
const NOW: usize = 0x18FC0; // the world's time
const POINT: usize = 0x18FC8; // the rollback point, -1 none
const FROZEN: usize = 0x18FDF; // waits for its first world

/// The pacing machine's state: the game has one world core at a time, and
/// one logic thread moves it.
static mut PACING: Pacing = Pacing::Free;

unsafe fn field(core: *mut u8, at: usize) -> u32 {
    unsafe { read_volatile(core.add(at) as *const u32) }
}

/// The render move of game_world::tick: `move(core, target)` as the game
/// does it, a rollback due within 50 ms of the last postponed (pacing).
///
/// # Safety
/// `core` is the game's world core, on the logic thread.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sync_render_move(core: *mut u8, target: u32) {
    unsafe {
        let world = WorldView {
            core: core as u32,
            now: field(core, NOW),
            point: field(core, POINT),
            frozen: read_volatile(core.add(FROZEN)) != 0,
        };
        let state = &raw mut PACING;
        let (next, plan) = pacing::render_move(*state, world, target);
        *state = next;
        match plan {
            Plan::Stock => engine::engine_world_core_move(core as *mut c_void, target),
            Plan::Postpone { keep } => {
                let point = core.add(POINT) as *mut u32;
                write_volatile(point, pacing::NO_POINT);
                engine::engine_world_core_move(core as *mut c_void, target);
                write_volatile(point, pacing::restored_point(keep, read_volatile(point)));
            }
        }
    }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    // A loud crash at a known address, never a wrong world.
    loop {
        unsafe { core::arch::asm!("ud2", options(noreturn)) }
    }
}
