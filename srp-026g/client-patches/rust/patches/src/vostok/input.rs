//! The player's input: survarium::player_input_handler, which turns the
//! keys' events into the input sent each tick.

use core::ffi::c_void;
use core::marker::PhantomData;
use core::ptr::NonNull;

use super::{FromHook, Hook};
use crate::raw;

/// A game action (the game's action table).
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// commit_suicide, default key K.
    CommitSuicide = 0x1F,
}

/// What the input handler holds of an action: its last key event, or
/// [`Up`](ActionState::Up) for one held from before.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionState {
    Down,
    Repeat,
    Up,
}

/// A state the game gave that is none of [`ActionState`]'s.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnknownActionState(pub u32);

impl ActionState {
    pub fn parse(raw: u32) -> Result<ActionState, UnknownActionState> {
        match raw {
            0 => Ok(ActionState::Down),
            1 => Ok(ActionState::Repeat),
            2 => Ok(ActionState::Up),
            other => Err(UnknownActionState(other)),
        }
    }
}

/// Bits of an input's action mask.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActionMask(u32);

impl ActionMask {
    /// The player kills themselves: base_player::process_quick_slots hits
    /// their own head.
    pub const SUICIDE: ActionMask = ActionMask(1 << 24);
}

/// The input handler, while it builds an input.
pub struct InputHandler<'h> {
    this: NonNull<c_void>,
    hook: PhantomData<&'h ()>,
}

impl<'h> FromHook<'h> for InputHandler<'h> {
    unsafe fn from_hook(_: &'h Hook<'_>, raw: u32) -> Option<Self> {
        NonNull::new(raw as usize as *mut c_void).map(|this| InputHandler { this, hook: PhantomData })
    }
}

impl InputHandler<'_> {
    /// The state of `action`: `None` when the handler has none, or when it
    /// is no [`ActionState`].
    pub fn action_state(&self, action: Action) -> Option<ActionState> {
        let mut state = 0;
        // SAFETY: `this` is the game's input handler (FromHook's contract),
        // on the game's thread inside a hook (the Hook it borrows);
        // action_present reads the handler and writes `state` alone.
        let present = unsafe { raw::engine_action_present(self.this.as_ptr(), action as u32, &mut state) };
        if present == 0 {
            return None;
        }
        ActionState::parse(state).ok()
    }

    /// Adds `actions` to the mask of the input being built.
    pub fn add_actions(&mut self, actions: ActionMask) {
        // SAFETY: as for action_state; the mask is the handler's own, which
        // the input builder that called the hook goes on filling after it.
        unsafe { *raw::player_input_handler::actions(self.this.as_ptr()) |= actions.0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn states_parsed_and_refused() {
        assert_eq!(ActionState::parse(0), Ok(ActionState::Down));
        assert_eq!(ActionState::parse(2), Ok(ActionState::Up));
        assert_eq!(ActionState::parse(7), Err(UnknownActionState(7)));
    }
}
