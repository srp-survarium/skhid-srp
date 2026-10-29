//! The player's input: survarium::player_input_handler, which turns the
//! keys' events into the input sent each tick.

use core::marker::PhantomData;

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

/// Bits of an input's action mask (the simulation reads them): the bits
/// process_first_person_mode sets per action (client_patch.md section 3.58).
/// A set of bits; the game's unmodelled ones pass through unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActionMask(u32);

impl ActionMask {
    pub const NONE: ActionMask = ActionMask(0);
    pub const FORWARD: ActionMask = ActionMask(0x1);
    pub const BACK: ActionMask = ActionMask(0x2);
    pub const LEFT: ActionMask = ActionMask(0x4);
    pub const RIGHT: ActionMask = ActionMask(0x8);
    pub const JUMP: ActionMask = ActionMask(0x10);
    pub const FIRE: ActionMask = ActionMask(0x20);
    pub const RELOAD: ActionMask = ActionMask(0x40);
    pub const AIM: ActionMask = ActionMask(0x80);
    pub const CROUCH: ActionMask = ActionMask(0x100);
    pub const SPRINT: ActionMask = ActionMask(0x200);
    /// The six quick-use slots, 4000h..80000h.
    pub const QUICK_USE: ActionMask = ActionMask(0x3F << 14);
    pub const HOLD_BREATH: ActionMask = ActionMask(1 << 20);
    pub const USE: ActionMask = ActionMask(1 << 21);
    pub const DROP: ActionMask = ActionMask(1 << 22);
    pub const GRENADE: ActionMask = ActionMask(1 << 23);
    /// The player kills themselves: base_player::process_quick_slots hits
    /// their own head.
    pub const SUICIDE: ActionMask = ActionMask(1 << 24);
    pub const MELEE: ActionMask = ActionMask(1 << 27);

    pub const fn union(self, other: ActionMask) -> ActionMask {
        ActionMask(self.0 | other.0)
    }

    pub const fn intersection(self, other: ActionMask) -> ActionMask {
        ActionMask(self.0 & other.0)
    }

    pub const fn without(self, other: ActionMask) -> ActionMask {
        ActionMask(self.0 & !other.0)
    }

    pub const fn bits(self) -> u32 {
        self.0
    }

    pub const fn from_bits(bits: u32) -> ActionMask {
        ActionMask(bits)
    }
}

/// The input handler, while it builds an input. A [`Handle`]: the generated
/// shim makes it from a hook's argument; a patch receives it.
pub struct InputHandler<'t> {
    this: raw::PlayerInputHandler,
    thread: PhantomData<&'t Thread>,
}

impl<'t> Handle<'t> for InputHandler<'t> {
    type Raw = raw::PlayerInputHandler;

    unsafe fn from_raw(_: &'t Thread, raw: Option<raw::PlayerInputHandler>) -> Option<Self> {
        raw.map(|this| InputHandler { this, thread: PhantomData })
    }
}

impl<'t> InputHandler<'t> {
    /// At `input_builder.end` (process_first_person_mode's epilogue).
    pub fn at_builder_end(ctx: &'t super::Context<'_>) -> Option<InputHandler<'t>> {
        // SAFETY: the context is this site's: the handler building the input,
        // alive until the epilogue returns, on the hook's thread.
        let this = unsafe { raw::site::input_builder_end::handler(ctx) }?;
        Some(InputHandler { this, thread: PhantomData })
    }
}

impl InputHandler<'_> {
    pub(in crate::vostok) fn raw(&self) -> raw::PlayerInputHandler {
        self.this
    }

    /// The state of `action`: `None` when the handler has none, or when it
    /// is no [`ActionState`].
    pub fn action_state(&self, action: Action) -> Option<ActionState> {
        let mut state = 0;
        // SAFETY: the handler is alive for the hook, on the game's thread (the
        // Thread it borrows); action_present reads it and writes `state` alone.
        let present = unsafe { raw::engine_action_present(Some(self.this), action.0, &mut state) };
        if present == 0 {
            return None;
        }
        ActionState::parse(state).ok()
    }

    /// The action mask of the input being built.
    pub fn actions(&self) -> ActionMask {
        // SAFETY: the handler is alive for the hook, on its thread.
        ActionMask(unsafe { *self.this.actions() })
    }

    pub fn set_actions(&mut self, actions: ActionMask) {
        // SAFETY: as actions.
        unsafe { *self.this.actions() = actions.0 }
    }

    /// Adds `actions` to the mask of the input being built.
    pub fn add_actions(&mut self, actions: ActionMask) {
        // SAFETY: as actions; the mask is the handler's own, which the input
        // builder goes on filling after the hook.
        unsafe { *self.this.actions() |= actions.0 }
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
