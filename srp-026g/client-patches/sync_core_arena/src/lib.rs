//! Patch 74: the client side of the sync core (`docs/notes/sync_core.md`).
//!
//! `sync_core_026g.asm`'s `sc_render_move` stands in for game_world::tick's
//! call of `game_world_core::move` (0x1F7C37), aligns the stack and calls
//! [`sync_render_move`]; the decision is `sync_core::pacing`'s, the move the
//! game's own through the engine library. This file is the only code that
//! knows the core's raw fields: it parses them into a `World` once and
//! encodes the one it writes back.

#![no_std]

use core::ffi::c_void;
use core::ptr::{read_volatile, write_volatile};

use sync_core::GameTime;
use sync_core::pacing::{self, CoreId, Pacing, Plan, World};

mod engine {
    include!("../../engine_026g.rs");
}

/// game_world_core's fields (sync_core.md 1.1).
const NOW: usize = 0x18FC0; // the world's time
const ROLLBACK: usize = 0x18FC8; // the rollback point, -1 none
const FROZEN: usize = 0x18FDF; // waits for its first world (a byte, not 0)

/// The rollback point's encoding: -1 is none.
const NO_ROLLBACK: u32 = u32::MAX;

/// The pacing machine's state: the game has one world core at a time, and
/// one logic thread moves it.
static mut PACING: Pacing = Pacing::Free;

/// The game's world core.
struct Core(*mut u8);

impl Core {
    unsafe fn word(&self, at: usize) -> u32 {
        unsafe { read_volatile(self.0.add(at) as *const u32) }
    }

    fn id(&self) -> CoreId {
        CoreId(self.0 as u32)
    }

    /// The core's rollback point, parsed.
    unsafe fn rollback(&self) -> Option<GameTime> {
        match unsafe { self.word(ROLLBACK) } {
            NO_ROLLBACK => None,
            ms => Some(GameTime(ms)),
        }
    }

    /// The rollback point, encoded.
    unsafe fn set_rollback(&self, rollback: Option<GameTime>) {
        let raw = rollback.map_or(NO_ROLLBACK, |t| t.0);
        unsafe { write_volatile(self.0.add(ROLLBACK) as *mut u32, raw) }
    }

    /// The world as the pacing reads it: the one parse of the raw fields.
    unsafe fn world(&self) -> World {
        unsafe {
            if read_volatile(self.0.add(FROZEN)) != 0 {
                World::Frozen { core: self.id() }
            } else {
                World::Running {
                    core: self.id(),
                    now: GameTime(self.word(NOW)),
                    rollback: self.rollback(),
                }
            }
        }
    }

    unsafe fn stock_move(&self, target: GameTime) {
        unsafe { engine::engine_world_core_move(self.0 as *mut c_void, target.0) }
    }
}

/// The render move of game_world::tick: `move(core, target)` as the game
/// does it, a rollback due within 50 ms of the last postponed (pacing).
///
/// # Safety
/// `core` is the game's world core, on the logic thread.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sync_render_move(core: *mut u8, target: u32) {
    unsafe {
        let core = Core(core);
        let target = GameTime(target);
        let state = &raw mut PACING;
        let (next, plan) = pacing::render_move(*state, core.world(), target);
        *state = next;
        match plan {
            Plan::Stock => core.stock_move(target),
            Plan::Postpone { keep } => {
                core.set_rollback(None);
                core.stock_move(target);
                core.set_rollback(Some(pacing::restored_point(keep, core.rollback())));
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
