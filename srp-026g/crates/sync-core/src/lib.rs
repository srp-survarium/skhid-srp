//! The 0.26g sync core: the state machines and transition rules that decide
//! what a match simulates when (`docs/notes/sync_core.md`).
//!
//! `no_std`, no allocation: the same code runs on the server (an ordinary
//! dependency) and in the patched client's arena (`client-patches/
//! sync_core_arena`, linked into `.srpx` by `link_026g.py`). Each machine
//! is one enum whose variants carry exactly the data valid in that state,
//! and one transition function that is total over its events.

#![no_std]

pub mod pacing;
pub mod time;

pub use time::GameTime;

/// The simulation's step: every world in the history is on this grid.
pub const TICK_MS: u32 = 50;

/// The client's event horizon (`game_world_core` +18FB4h): the item this
/// far behind the newest time ticked is the one 0xDF hashes and 0xF6
/// uploads.
pub const EVENT_HORIZON_MS: u32 = 500;
