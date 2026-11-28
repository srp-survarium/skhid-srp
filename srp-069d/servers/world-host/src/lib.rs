//! In-process host for the 0.69d game exe: runs the exe's own world code for online matches.
//!
//! The exe (x86-64 PE, fixed image base 0x200000000, no DYNAMIC_BASE) is mapped by our own loader,
//! its import tables (KERNEL32, vostok_scaleform and the 19 delay-import DLLs) point at host stubs,
//! every thread that runs exe code gets a fake TEB/PEB through the GS base, and exe functions are
//! called with the Win64 ABI (`extern "win64"`). All of this is Linux x86-64 only.
#![cfg(all(target_os = "linux", target_arch = "x86_64"))]
#![allow(clippy::missing_safety_doc)]

pub mod base;
pub mod baseset;
pub mod bots;
pub mod bp;
pub mod cmdline;
pub mod crash;
pub mod crypt;
pub mod d3d;
pub mod daemon;
pub mod dinput;
pub mod fdpass;
pub mod files;
pub mod handles;
pub mod k32;
pub mod kills;
pub mod log;
pub mod mem;
pub mod nullcook;
pub mod other;
pub mod pagemap;
pub mod pe;
pub mod registry;
pub mod run;
pub mod session;
pub mod strings;
pub mod teb;
pub mod thunks;
pub mod vitals;
pub mod world;
pub mod xaudio;
