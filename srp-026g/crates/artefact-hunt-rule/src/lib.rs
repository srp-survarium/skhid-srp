//! Artifact Hunt: the mode's rules, apart from the game.
//!
//! 0.46c2's `artefact_hunt_rule_core` and `victory_artefact_core`, written
//! again as one state machine over integers (`docs/notes/artefact_hunt.md`
//! the rules, `artefact_hunt_046c2.md` the donor's code). The game's side (an
//! exe patch) turns the world into [`Event`]s, applies the [`Action`]s, asks
//! the state at the game's own decisions (a death's respawn time
//! [`Rule::respawn_at`], the places due [`Rule::spawning`], the inputs
//! [`Rule::frozen`], the carrier [`Rule::carrier`]) and shows it
//! ([`Rule::hud`], [`Cue`]); this crate holds no pointer into the game, keeps no
//! static storage and is tested on the host (`cargo test -p
//! artefact-hunt-rule`).
//!
//! `no_std`, no allocation, no float arithmetic that a match depends on but
//! one multiply a coordinate (a position is turned into whole millimetres
//! once, at the boundary), and no 64-bit division: every game computes the
//! same bytes.

#![no_std]
#![forbid(unsafe_code)]

mod cue;
mod geometry;
mod params;
mod roster;
mod rule;
mod shape;
mod time;
mod way;
mod wire;

pub use cue::{Cue, Line, Message, Viewer};
pub use geometry::Position;
pub use params::{
    BASE_MARGIN, BaseDef, FADE, MAX_SHAPES, MAX_SPAWNERS, ParamError, Params, REACH, RawParams,
    RawShape, Spawner,
};
pub use roster::{PLAYERS, Place, Roster, Sides, Team};
pub use rule::{
    Action, Actions, After, Artefact, ArtefactView, Clock, Event, Hud, MAX_ACTIONS, Outcome, Over,
    Phase, Play, Reason, Rng, Round, Rule, Shift, Spawning, Spot, Stage, Sudden, SuddenShown, Wave,
};
pub use shape::{Shape, ShapeKind, View, along};
pub use time::{Ms, Time};
pub use way::{MAX_WAY, WHOLE, Way};
pub use wire::{Field, MAX_BYTES, Value, WireError};

#[cfg(test)]
mod tests;
