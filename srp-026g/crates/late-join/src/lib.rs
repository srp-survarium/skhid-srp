//! The late join's logic on the client (exe patch 11): what the server's
//! E4 extension proposes, checked and owned until the game's own loader has
//! made the newcomer's players (`client-patches/late_join_026g.asm`, the
//! hooks and the game's calling conventions; `client-patches/
//! late_join_payload`, this crate built into the exe and the native install).
//!
//! `no_std`, no allocation of its own (the game's allocator through the
//! [`runtime::Api`]), no static storage: the state lives in the exe's state
//! page and is handed in. Tested on the host (`cargo test -p late-join`).

#![no_std]

pub mod runtime;
pub mod validate;

#[cfg(test)]
extern crate std;
