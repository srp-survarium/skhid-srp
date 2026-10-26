//! The player's input: survarium::player_input_handler, which turns the
//! keys' events into the input sent each tick.

use core::ffi::c_void;
use core::marker::PhantomData;
use core::ptr::NonNull;

use super::hook::{Handle, Thread};
use crate::raw;

/// A game action (an entry of the action table). A newtype, not an enum:
/// the game passes actions a patch does not model (spot is `0x22`), and a
/// hooked function hands those on unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Action(pub u32);

impl Action {
    /// commit_suicide, default key K (action table entry 0xA74D20).
    pub const COMMIT_SUICIDE: Action = Action(0x1F);
    /// spot.
    pub const SPOT: Action = Action(0x22);
}

/// What the input handler holds of an action: its last key event.
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

/// Bits of an input's action mask (the simulation reads them).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActionMask(u32);

impl ActionMask {
    /// The player kills themselves: base_player::process_quick_slots hits
    /// their own head.
    pub const SUICIDE: ActionMask = ActionMask(1 << 24);
}

/// The input handler, while it builds an input. A [`Handle`]: the generated
/// shim makes it from a hook's argument; a patch receives it.
pub struct InputHandler<'t> {
    this: NonNull<c_void>,
    thread: PhantomData<&'t Thread>,
}

impl<'t> Handle<'t> for InputHandler<'t> {
    unsafe fn from_raw(_: &'t Thread, raw: u32) -> Option<Self> {
        NonNull::new(raw as usize as *mut c_void).map(|this| InputHandler { this, thread: PhantomData })
    }
}

impl InputHandler<'_> {
    pub(in crate::vostok) fn ptr(&self) -> *mut c_void {
        this_ptr(self.this)
    }

    /// The state of `action`: `None` when the handler has none, or when it
    /// is no [`ActionState`].
    pub fn action_state(&self, action: Action) -> Option<ActionState> {
        let mut state = 0;
        // SAFETY: `this` is the game's input handler (Handle's contract), on
        // the game's thread (the Thread it borrows); action_present reads
        // the handler and writes `state` alone.
        let present = unsafe { raw::engine_action_present(self.ptr(), action.0, &mut state) };
        if present == 0 {
            return None;
        }
        ActionState::parse(state).ok()
    }

    /// Adds `actions` to the mask of the input being built.
    pub fn add_actions(&mut self, actions: ActionMask) {
        // SAFETY: as action_state; the mask is the handler's own field,
        // which the input builder goes on filling after the hook.
        unsafe { *raw::player_input_handler::actions(self.ptr()) |= actions.0 }
    }
}

fn this_ptr(this: NonNull<c_void>) -> *mut c_void {
    this.as_ptr()
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
