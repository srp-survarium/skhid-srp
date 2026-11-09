//! The Sponge (exe patch 59, artefact type 12, items_dict 758): 0.28d0's
//! thrown toxic artefact on 0.26g, thrown and exploding as retail 0.69d0's
//! (docs/notes/sponge.md). This crate is its rules, with no engine pointer:
//!
//! - [`machine`]: what a sponge is (one state, the data of each its own)
//!   and the one transition from (state, event) to the new state and what
//!   the game must do;
//! - [`record`]: its fields in a world (after artefact_base's record), parsed
//!   once into a [`record::Saved`] and written from one: the client patch
//!   writes and reads through it, the world check (srp-validate) parses with it;
//! - [`spots`]: where it spawns (the owner's spots, each map's mode configs'
//!   `mode/artefact_spots`): the cores' picked containers and spawn_artefact's
//!   draw.
//!
//! The client patch (client-patches/rust/patches: vostok::sponge, the
//! patch module sponge) keeps a [`machine::State`] in each sponge object and
//! drives it from the game's hooks.

#![no_std]
#![forbid(unsafe_code)]

pub mod machine;
pub mod record;
pub mod spots;

/// The Sponge's items_dict id.
pub const KIND: u32 = 758;

/// The places of a match (the world's place table: 20).
pub const PLACES: u8 = 20;

/// The world's tick grid: 50 ms.
pub const TICK: u32 = 50;

/// A world's simulation time, ms (it wraps: compare by difference).
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GameTime(pub u32);

impl GameTime {
    /// `self` + `ms`, wrapping.
    pub const fn after(self, ms: u32) -> GameTime {
        GameTime(self.0.wrapping_add(ms))
    }

    /// How far `self` is after `other`, signed (the fuse's rule compares so).
    pub const fn since(self, other: GameTime) -> i32 {
        self.0.wrapping_sub(other.0) as i32
    }
}

/// A place of the match (below [`PLACES`]): who threw a sponge.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Place(u8);

impl Place {
    pub const fn new(place: u8) -> Option<Place> {
        if place < PLACES { Some(Place(place)) } else { None }
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}
