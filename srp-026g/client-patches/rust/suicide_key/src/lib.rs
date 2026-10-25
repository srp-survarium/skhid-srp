//! Patch 34: the commit_suicide key (K) kills one's own player (build 2777;
//! client_patch.md section 3.32). The framework's example patch
//! (`docs/notes/rust_patches.md`): its NASM entry is suicide_key_026g.asm,
//! one `RUST_HOOK` and the replaced call.
//!
//! commit_suicide is game action 1Fh (action table entry 0xA74D20, default
//! key kK). A key bound to it already reaches the player input handler's
//! event buffer (0x23FEAC), but the input builder 0x241250, which turns the
//! buffered actions into the input's action mask (handler+33Ch), never asks
//! for 1Fh, and bit 24 is the one bit it never sets. The match simulation
//! still reads it: base_player::process_quick_slots (0x2D8BC1, `test dword
//! [edi+4], 1000000h`) hits the player's own head bone "brain" through
//! hit_impl (0x2D9AE0 -> 0x2D9B90), the attacker being the player. It is the
//! relayed input every game applies to every player, so every game does the
//! same.
//!
//! 0x242196 `call 0x242460` (spot, the builder's last action; ebx = the
//! handler, pushed with action 22h and &state, the caller's [esp+44h]) ->
//! `call suicide_key`. Action 1Fh is asked the same way first; on the key
//! going down (state 0, as spot) bit 24 is set. Then the original call goes
//! on with the same arguments and return address (the entry's `jmp`). When
//! spot is in the buffer it writes the state again; when it is not (al = 0)
//! the builder reads no state (0x24219D `jz 0x2421B0`), so nothing after it
//! sees this one.

#![cfg_attr(target_arch = "x86", no_std)]

use patch_core::Field;
#[cfg(target_arch = "x86")]
use patch_core::{GameFn, Regs, Va};

/// The game's player_input_handler.
pub enum InputHandler {}

/// The action mask of the input being built.
pub const ACTIONS: Field<InputHandler, u32> = Field::at(0x33C);
/// The input's suicide bit (process_quick_slots reads it).
pub const SUICIDE: u32 = 1 << 24;
/// The game's action commit_suicide.
pub const COMMIT_SUICIDE: u32 = 0x1F;
/// A state of a buffered action: the key went down.
pub const DOWN: u32 = 0;

/// player_input_handler::action_state(handler, action, &state) -> al: the
/// action's state in the event buffer (al 1, *state written) or none (al 0).
/// (stdcall exists on x86 only: the game's side is `target_arch = "x86"`.)
#[cfg(target_arch = "x86")]
pub const ACTION_STATE: GameFn<unsafe extern "stdcall" fn(u32, u32, *mut u32) -> u8> = GameFn::at(Va(0x242460));

/// What the buffer said of commit_suicide.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Asked {
    /// Not in the buffer.
    Absent,
    /// In it, with this state (0 down, 1 repeat, 2 up).
    State(u32),
}

/// Whether the input sets the suicide bit: the key went down.
pub fn kills(asked: Asked) -> bool {
    asked == Asked::State(DOWN)
}

/// The hook (suicide_key_026g.asm's RUST_HOOK): EBX the handler, the
/// game's stack at the entry: [0] the return address, [1] the handler,
/// [2] action 22h, [3] &state.
///
/// # Safety
/// The input builder's call at 0x242196, on the game's thread.
#[cfg(target_arch = "x86")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn suicide_key_hook(regs: &mut Regs) {
    unsafe {
        let state = regs.stack(3) as *mut u32;
        let asked = match ACTION_STATE.get()(regs.ebx, COMMIT_SUICIDE, state) {
            0 => Asked::Absent,
            _ => Asked::State(*state),
        };
        if kills(asked) {
            let handler = patch_core::Obj::<InputHandler>::at(Va(regs.ebx));
            if let Some(handler) = handler {
                handler.set(ACTIONS, handler.get(ACTIONS) | SUICIDE);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_key_going_down_kills() {
        assert!(kills(Asked::State(DOWN)));
        assert!(!kills(Asked::State(1)));
        assert!(!kills(Asked::State(2)));
        assert!(!kills(Asked::Absent));
    }
}
