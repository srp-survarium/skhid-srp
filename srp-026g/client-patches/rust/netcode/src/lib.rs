//! The per-tick protocol's client (patch 79; `docs/notes/
//! netcode_port_069.md` phases 2 and 3; the server's side is match-server's
//! `game/ticks.rs`, the shared rules `sync_core::{wire, confirm, clock}`).
//!
//! With a server that plays it (0xE4's trailer, [`trailer`]):
//! - the game's own inputs go to the server in one update a tick (0xE2),
//!   empty or not, instead of the stock 0xDC/0xDD/0xDE: the stock sender
//!   still picks what to send, and the patch takes it at its new_packet
//!   ([`machine::Netcode::own_input`]);
//! - the server's update of a tick (0xF7) is made into the stock messages
//!   that do each of its changes ([`stock`]), handed to the game's own
//!   dispatch: its outcomes of the own updates, the roster changes, the
//!   others' inputs; and the clock is steered from how early the updates
//!   came (a 0xE7 of the game's own handler).
//!
//! With any other server (no trailer) the patch does nothing: the stock
//! protocol, byte for byte. The game announces the protocol in its 0xD8
//! (bit 1 of the capability byte: `netcode_026g.asm`), and knows the two
//! new ids in its tables either way.
//!
//! The logic ([`machine`], [`stock`], [`trailer`]) is plain and tested on
//! the host; the game's side is `game` (`target_arch = "x86"`).

#![cfg_attr(target_arch = "x86", no_std)]

pub mod machine;
pub mod stock;
pub mod trailer;

#[cfg(target_arch = "x86")]
mod game;
