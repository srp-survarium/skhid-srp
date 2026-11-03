//! Allocation-free match-message validation shared by server and client.
//! Extracted from PR #478; runtime integration is a separate change.

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
pub mod recovery;
pub mod reader;
pub mod refusal;
pub mod transport;

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
