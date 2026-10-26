//! The death ragdoll of the patched 0.26g client (`docs/notes/ragdoll.md`):
//! a visual-only XPBD ragdoll that the simulation never sees.
//!
//! `no_std`, no allocation, no engine pointer: the same code is unit-tested
//! here and runs in the client's arena (`client-patches/ragdoll_arena`,
//! linked into `.srpx` by `link_026g.py`), which reads the game's memory
//! into these types once and writes the drawn bones back.
//!
//! - [`ragdoll`]: one ragdoll, its bodies, joints and limits, the step and
//!   the drawn skeleton;
//! - [`corpse`]: the state machine of each place's corpse (Alive, Dying,
//!   Ragdoll, Settled), keyed per death, and the pool of ragdolls;
//! - [`camera`]: the death camera's follow of a tumbling head.

#![no_std]
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
