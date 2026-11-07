//! The death ragdoll of the patched 0.26g client (`docs/notes/ragdoll.md`):
//! a visual-only XPBD ragdoll that the simulation never sees.
//!
//! `no_std`, no allocation, no engine pointer: the same code is unit-tested
//! here and runs in the client's arena: the Rust patch `ragdoll`
//! (`client-patches/rust/patches/src/ragdoll.rs`, over vostok; linked into
//! `.srpx` by `link_026g.py`) reads the game's memory into these types once
//! a frame and writes the drawn bones back.
//!
//! - [`ragdoll`]: one ragdoll, its bodies, joints and limits, the step and
//!   the drawn skeleton;
//! - [`corpse`]: the state machine of each place's corpse (Alive, Dying,
//!   Ragdoll, Settled), keyed per death, and the pool of ragdolls;
//! - [`camera`]: the death camera's follow of a tumbling head.

#![no_std]
#![forbid(unsafe_code)]
#![feature(core_float_math)]

pub mod body;
pub mod camera;
pub mod corpse;
pub mod limits;
pub mod math;
pub mod ragdoll;
pub mod shot;
pub mod skeleton;
pub mod tuning;
