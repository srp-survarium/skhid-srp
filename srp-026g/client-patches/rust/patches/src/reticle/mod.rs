//! A collimator's reticle chosen per scope (`docs/notes/reticle-choice.md`,
//! "The build's contract"): the table of the account's choices (`table`,
//! the lobby's message) and the drawing of the chosen pattern in the local
//! player's aimed optic (`draw`).

#![forbid(unsafe_code)]

pub mod draw;
pub mod table;

pub use draw::{on_culled, on_draw, on_hidden, on_loaded, on_release, on_tick};
