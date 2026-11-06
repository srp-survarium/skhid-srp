//! The player's input: survarium::player_input_handler, which turns the
//! keys' events into the input sent each tick, and the input as a world takes
//! it (game_world_core::change_input<player_input>, [`InputChange`]).

use core::marker::PhantomData;

use super::context::{Context, Reg};
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
    /// artefact (action table entry 0xA74D5C): unused by the game; patch 78
    /// gives it the default key H and a controls row (patch 62's table), and
    /// holding it makes a quick-use key offer that slot's artefact.
    pub const ARTEFACT: Action = Action(0x23);
    /// mute_last_speaker: the action table's debug entry serialize_player_state
    /// (0xA74E10), renamed and made a chat key (kind 8) by patch 66.
    pub const MUTE_LAST_SPEAKER: Action = Action(0x37);
    /// ptt (entry 0xA74F8C, kind 8, default P): patch 66's push-to-talk.
    pub const PTT: Action = Action(0x47);
}

/// What the input handler holds of an action: its last key event, as
/// on_keyboard_action pushes it (0.10b `player_input_handler::action_state_enum`:
/// down 0, up 1, hold 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionState {
    Down,
    Up,
    Hold,
}

/// A state the game gave that is none of [`ActionState`]'s.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnknownActionState(pub u32);

impl ActionState {
    pub fn parse(raw: u32) -> Result<ActionState, UnknownActionState> {
        match raw {
            0 => Ok(ActionState::Down),
            1 => Ok(ActionState::Up),
            2 => Ok(ActionState::Hold),
            other => Err(UnknownActionState(other)),
        }
    }
}

/// A key's event as player_input_handler::on_keyboard_action is given it
/// (its third argument; 0.10b `input::enum_keyboard_action`: down 1, up 2,
/// hold 3, the game pushing them as [`ActionState`]'s 0, 1, 2 at 0x23FEA4,
/// 0x23FF14, 0x23FED8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyboardAction {
    Down,
    Up,
    Hold,
}

impl KeyboardAction {
    pub fn parse(raw: u32) -> Option<KeyboardAction> {
        match raw {
            1 => Some(KeyboardAction::Down),
            2 => Some(KeyboardAction::Up),
            3 => Some(KeyboardAction::Hold),
            _ => None,
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

/// An input's rotation (`math::half2`): two IEEE half floats, the yaw's and
/// the pitch's turn, as they came. Never decoded: every world adds the halves
/// as they are, so a re-encoded one would be another simulation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rotation(u32);

/// A half float's exponent field.
const HALF_EXPONENT: u16 = 0x7C00;
/// The exponent of 1024.0: a rotation of 163 turns in one input.
const HALF_EXPONENT_1024: u16 = 25 << 10;

impl Rotation {
    pub fn from_bits(bits: u32) -> Rotation {
        Rotation(bits)
    }

    pub fn bits(self) -> u32 {
        self.0
    }

    /// Whether a client's mouse can make it: both halves numbers, each under
    /// 1024 radians. The match server's rule (`PlayerInput::rotation_is_sane`,
    /// crates/survarium), which refuses every input 0.27d's change_input
    /// refuses (its validator 0x1EA990: a half that is a NaN or an infinity)
    /// and those whose sum of two merged into one node (0x2C5B60) could reach
    /// infinity.
    pub fn is_sane(self) -> bool {
        [self.0 as u16, (self.0 >> 16) as u16].iter().all(|half| half & HALF_EXPONENT < HALF_EXPONENT_1024)
    }
}

/// game_world_core::change_input<player_input>'s event action (its result,
/// `event_action_enum`) for an input it refused: what both of 0.26g's own
/// refusals ("older than last resync", 0x1FAD10) and 0.27d's "input is
/// invalid" (0x1EFA3F) return. Its callers read nothing of a refusal.
const REJECTED: u32 = 6;

/// A player input at the start of a world's change_input (`change_input.player`,
/// 0x1FAB50): every input a world takes goes through it, a local one, a
/// relayed one, a moved one and a replay's.
pub struct InputChange<'c, 'm> {
    ctx: &'c mut Context<'m>,
    rotation: Rotation,
    time: u32,
    place: u8,
}

impl<'c, 'm> InputChange<'c, 'm> {
    pub fn at(ctx: &'c mut Context<'m>) -> Option<InputChange<'c, 'm>> {
        use raw::site::change_input_player as site;
        // SAFETY: the context is this site's: its ESP the function's
        // arguments, alive while it runs on the game's thread.
        let (rotation, time, place) = unsafe {
            let frame = site::frame(&*ctx)?;
            (*frame.rotation(), *frame.time(), *frame.player())
        };
        Some(InputChange { ctx, rotation: Rotation(rotation), time, place })
    }

    pub fn rotation(&self) -> Rotation {
        self.rotation
    }

    pub fn time(&self) -> u32 {
        self.time
    }

    pub fn place(&self) -> u8 {
        self.place
    }

    /// The world does not take it: nothing of it is kept. The caller then
    /// leaves by the site's `refused` exit (its `ret 24h`).
    pub fn refuse(self) {
        self.ctx.set_reg(Reg::Eax, REJECTED);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotations_a_mouse_makes_and_the_rest() {
        let both = |x: u16, y: u16| Rotation::from_bits(u32::from(x) | u32::from(y) << 16).is_sane();
        assert!(both(0, 0));
        assert!(both(0x3C00, 0xBC00)); // 1.0, -1.0
        assert!(both(0x63FF, 0xE3FF)); // 1023.5 and its negative
        assert!(both(0x0001, 0x8000)); // a subnormal, -0
        for wrong in [0x6400, 0xE400, 0x7BFF, 0x7C00, 0xFC00, 0x7E00, 0xFFFF] {
            // 1024, -1024, 65504, the infinities, NaNs
            assert!(!both(wrong, 0), "{wrong:#x} as the yaw");
            assert!(!both(0, wrong), "{wrong:#x} as the pitch");
        }
    }

    #[test]
    fn states_parsed_and_refused() {
        assert_eq!(ActionState::parse(0), Ok(ActionState::Down));
        assert_eq!(ActionState::parse(1), Ok(ActionState::Up));
        assert_eq!(ActionState::parse(2), Ok(ActionState::Hold));
        assert_eq!(ActionState::parse(7), Err(UnknownActionState(7)));
    }

    #[test]
    fn keyboard_actions_are_the_engines() {
        assert_eq!(KeyboardAction::parse(1), Some(KeyboardAction::Down));
        assert_eq!(KeyboardAction::parse(2), Some(KeyboardAction::Up));
        assert_eq!(KeyboardAction::parse(3), Some(KeyboardAction::Hold));
        assert_eq!(KeyboardAction::parse(0), None);
    }
}
