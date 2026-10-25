//! What every Rust exe patch uses (`docs/notes/rust_patches.md`), and no
//! game logic.
//!
//! - [`mem`]: the game's memory through typed views: an address ([`Va`]), a
//!   field of an object kind ([`Field`]), an object ([`Obj`]), a global
//!   ([`Global`]), a game function ([`GameFn`]) and an import ([`Import`]);
//!   patch code names offsets once, as `const` fields, and never adds them up.
//!
//! `no_std`, no allocator. The game's build is link_026g.py's (i686,
//! opt-level s, panic=abort, fat LTO); the host's is `cargo test`, where what
//! only means something in the game is left out (`target_arch = "x86"`).

#![no_std]

#[cfg(test)]
extern crate std;

pub mod mem;

pub use mem::{Field, GameFn, Global, Import, Obj, Va};
