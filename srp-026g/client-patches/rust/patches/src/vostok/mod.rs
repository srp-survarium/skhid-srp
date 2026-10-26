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
//! - state shared between hooks on several of the game's threads (the
//!   draws, the bullets) is a [`Lock`]; the ragdoll's world ([`world`]), the
//!   draw's sites ([`draw`]), the camera's ([`camera`]), a bullet's ray
//!   ([`bullet`]), the machine's clock ([`clock`]) and the opt-in
//!   diagnostics ([`trace`]) are patch 76's.
//!
//! It grows with what the patches need, and no further.

pub mod bullet;
pub mod camera;
mod cell;
pub mod clock;
pub mod config;
mod chat;
pub mod console;
mod context;
pub mod draw;
mod damage;
mod gfx;
mod hook;
mod input;
pub mod kill_feed;
pub mod kernel32;
mod object;
mod keyboard;
pub mod log;
mod options;
pub mod parse;
pub mod replay;
pub mod render;
pub mod player;
pub mod spawn;
pub mod sign_in;
pub mod spotting;
mod rt;
mod sync;
pub mod trace;
pub mod types;
pub mod world;
mod ui;
pub mod world_input;

/// The generated hook glue (link_026g.py from the patches' TOML): each
/// hook's `extern "C"` shim and its `Original`. Inside `vostok` so it reaches
/// the typed primitives the handles and `Original`s are built from.
mod generated {
    // which of these each hook uses depends on its kind; the shipped set is
    // one `call`, so some are unused until another kind ships.
    #[allow(unused_imports)]
    use super::context::{Context, Flow, Frame};
    #[allow(unused_imports)]
    use super::hook::{Handle, Out, Thread};
    #[allow(unused_imports)]
    use super::chat::{Channel, ChatHandler, ChatText};
    #[allow(unused_imports)]
    use super::input::{Action, InputHandler};
    #[allow(unused_imports)]
    use crate::raw;
    include!("../../../../hooks_026g.rs");
}

pub use cell::GameCell;
pub use sync::Lock;
pub use context::{Context, Exit, Flag, Flow, NoExit, Reg, Ret};
pub use chat::{Channel, ChatHandler, ChatText};
pub use damage::{BodyPart, PartName};
pub use gfx::Movie;
pub use generated::*;
pub use hook::{Handle, Out, Regs, Thread};
pub use input::{Action, ActionMask, ActionState, InputHandler, UnknownActionState};
pub use keyboard::TabTest;
pub use options::OptionsRead;
pub use rt::c_u32;
pub use ui::GameWorldUi;
