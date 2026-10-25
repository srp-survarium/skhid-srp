//! The engine-function library (`client-patches/engine_026g.tsv`, README
//! "Calling the game's functions"): one row per function of the game's, its
//! registers and stack as the re-kit's ABI rows prove them, and a cdecl
//! thunk for it in the arena's block `engine.code`. `engine_026g.py --write`
//! generates the declarations included here (`engine_026g.rs`), with the
//! types of the row's prototype; the link gives each its address.
//!
//! So a function of the game's is called one of three ways:
//! - a row here (any registers, the thunk does them): `engine::engine_malloc(..)`;
//! - a plain `cdecl`, `stdcall` or `thiscall` one, directly: a
//!   [`crate::GameFn`] constant of its type;
//! - an import (kernel32): an [`crate::Import`] constant of its IAT slot.
//!
//! A new function the game has in registers is a new row of the TSV (and of
//! the re-kit), never a hand-written call.

include!("../../../engine_026g.rs");
