//! What every Rust exe patch uses (`docs/notes/rust_patches.md`), and no
//! game logic.
//!
//! - [`mem`]: the game's memory through typed views: an address ([`Va`]), a
//!   field of an object kind ([`Field`]), an object ([`Obj`]), a global
//!   ([`Global`]), a game function ([`GameFn`]) and an import ([`Import`]);
//!   patch code names offsets once, as `const` fields, and never adds them up;
//! - [`hook`]: the hook ABI, the Rust side of `rust_hook_026g.inc`: the
//!   registers a hook entry saved ([`Regs`]);
//! - [`engine`]: the engine-function library's declarations
//!   (`engine_026g.rs`, generated from `engine_026g.tsv`);
//! - [`types`]: the units: [`GameTime`], [`Tick`], [`Place`];
//! - [`parse`]: bytes from the network or from memory read once into typed
//!   values ([`Reader`], [`Parse`]).
//!
//! `no_std`, no allocator. The game's build is link_026g.py's (i686,
//! opt-level s, panic=abort, fat LTO); the host's is `cargo test`, where what
//! only means something in the game is left out (`target_arch = "x86"`).

#![no_std]

#[cfg(test)]
extern crate std;

pub mod engine;
pub mod hook;
pub mod mem;
pub mod parse;
pub mod types;

pub use hook::Regs;
pub use mem::{Field, GameFn, Global, Import, Obj, Va};
pub use parse::{Parse, Reader};
pub use types::{GameTime, Place, Tick};
