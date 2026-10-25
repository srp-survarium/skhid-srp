//! Vostok, the game's engine, as a safe API over `raw`: what a patch may
//! touch, typed. The only `unsafe` blocks of the patches are here, each
//! saying why it holds.
//!
//! - a handle ([`InputHandler`]) is a game object a hook was given: made
//!   from a hook's argument alone ([`FromHook`]), it borrows the [`Hook`],
//!   so a patch can neither make one up nor keep it past the hook;
//! - the game's values are enums and newtypes ([`Action`], [`ActionState`],
//!   [`ActionMask`]); a value the game gives that is none of an enum's is a
//!   typed refusal, never a panic.
//!
//! It grows with what the patches need, and no further. The runtime is here
//! too: the hook context ([`Hook`], [`Regs`]), the [`log`], the panic
//! handler and memcpy/memset, [`c_u32`]; the units ([`types`]) and the
//! parsing of bytes ([`parse`]).

mod hook;
mod input;
pub mod log;
pub mod parse;
mod rt;
pub mod types;

pub use hook::{FromHook, Hook, Regs};
pub use input::{Action, ActionMask, ActionState, InputHandler, UnknownActionState};
pub use rt::c_u32;
