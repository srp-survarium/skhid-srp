//! The checks of what a 0.26g game reads from the match server, in one
//! piece of code that two builds share (`docs/notes/client_world_check.md`
//! section 8):
//! - the match server, before it relays a world a player uploaded
//!   (`survarium::v026g::match_msgs::world`, #282 and #289);
//! - the game itself, in the exe's arena (patch 75,
//!   `client-patches/client_verify_arena`), before its own reader takes a
//!   world or a message in.
//!
//! So the crate is `no_std` with no `alloc`, and has no `unsafe`: a client
//! allocator would need writable state and an out-of-memory path, and a
//! panic in the game is the crash the check is there to stop. What a check
//! needs of the match comes in as plain values ([`world::Context`]): the
//! server fills them from its match, the game from its own objects.
//!
//! Every read is bounds-checked ([`reader::Reader`]); a fault is a
//! [`Refusal`]: the part of the message, the offset in it and the reason,
//! as numbers. Its `Display` is the server's warning text,
//! `<part> at <offset>: <reason>`.
//!
//! - [`world`]: a world snapshot (0xE8, the upload 0xF6),
//!   `docs/notes/world_validation.md`.
//! - [`messages`]: the match messages' envelopes (0xE3..0xF6).
//! - [`transport`]: the LZMA2 output bound of the two bodies a client
//!   decompresses (0xE8, 0xE4), `world_validation.md` section 11.

#![no_std]
#![forbid(unsafe_code)]
// No panic is reachable in the game's build: every index and every sum is
// checked (the payload's link asserts the panic handler is gone).
#![cfg_attr(
    not(test),
    deny(
        clippy::indexing_slicing,
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::arithmetic_side_effects
    )
)]

// The tests run under the std test harness; the crate itself stays no_std.
#[cfg(test)]
extern crate std;

pub mod messages;
pub mod reader;
pub mod refusal;
pub mod transport;
pub mod world;

pub use refusal::{Part, Reason, Refusal};

/// The roster places of a match: the game's place table holds 20
/// (client_world_check.md section 7), and the masks of a world name them.
pub const MAX_PLAYERS: usize = 20;

/// The player objects of a place, one for each kit ("preset").
pub const KITS: usize = 3;

/// The game modes (`maps::mode` in the server).
pub mod mode {
    /// Batteries.
    pub const BATTERIES: u8 = 2;
    /// Batteries placed over time.
    pub const TIMED_BATTERIES: u8 = 3;
    /// Team Deathmatch.
    pub const TEAM_DEATHMATCH: u8 = 4;
    /// Research (exe patch 53, `research_mode.md`).
    pub const RESEARCH: u8 = 5;
    /// The modes whose statistics and gather rule hold the map's batteries.
    pub const WITH_BATTERIES: [u8; 2] = [BATTERIES, TIMED_BATTERIES];
}

/// The timelimit rule's states (`rules::state` in the server); a world's
/// is at most [`IN_PROCESS`](timelimit::IN_PROCESS).
pub mod timelimit {
    pub const IN_PROCESS: u8 = 3;
}

/// Whether the f32 of these bits is a number (not infinite, not NaN). On
/// the bits, so that the game's build has no float code.
pub const fn finite(bits: u32) -> bool {
    bits & 0x7F80_0000 != 0x7F80_0000
}
