//! Vostok, the game's engine, as a safe typed API over `raw`, plus the hook
//! system and the runtime. The only `unsafe` of the patches lives here, each
//! block saying why it holds; a patch module uses `vostok` alone.
//!
//! - a handle ([`InputHandler`]) is a game object a hook was given, built by
//!   a generated shim from a hook's argument ([`Handle`]); it borrows the
//!   hook's [`Thread`], so a patch cannot keep it past the hook nor make one
//!   on another thread, and cannot conjure one (every way is `unsafe`,
//!   which a patch forbids);
//! - the game's values are enums and newtypes ([`Action`], [`ActionState`],
//!   [`ActionMask`]); a value the game gives that fits none is a typed
//!   refusal, never a panic;
//! - the hook system: a `call`/`splice` handler gets a generated `Original`
//!   to call the game's own function; a `lohook` handler gets a [`Context`]
//!   and returns a [`Flow`]; state shared between hooks is a [`GameCell`].
//!
//! It grows with what the patches need, and no further.

mod cell;
mod context;
mod hook;
mod input;
pub mod log;
pub mod parse;
mod rt;
pub mod types;

/// The generated hook glue (link_026g.py from the patches' TOML): each
/// hook's `extern "C"` shim and its `Original`. Inside `vostok` so it reaches
/// the typed primitives the handles and `Original`s are built from.
mod generated {
    use super::hook::{Handle, Out, Thread};
    use super::input::{Action, InputHandler};
    use crate::raw;
    include!("../../../../hooks_026g.rs");
}

pub use cell::GameCell;
pub use context::{Context, Exit, Flag, Flow, NoExit, Reg, Ret};
pub use generated::*;
pub use hook::{Handle, Out, Regs, Thread};
pub use input::{Action, ActionMask, ActionState, InputHandler, UnknownActionState};
pub use rt::c_u32;
