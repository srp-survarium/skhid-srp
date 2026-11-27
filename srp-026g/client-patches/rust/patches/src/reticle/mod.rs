//! Patch `<patch>`: a collimator's reticle chosen per scope item (`docs/notes/reticle-choice.md`,
//! "The build's contract"). [`table`] keeps the server's `A9` for the process and serves the
//! lobby's window (`srp_set_reticle`, each scope item's `srp_reticle`); [`draw`] draws it in the
//! holder's first-person aimed view from [`table::drawn_for_dict`].

pub mod draw;
pub mod table;

pub use draw::{on_culled, on_draw, on_hidden, on_loaded, on_release, on_tick};
pub use table::{on_inventory_item, on_lobby_call, on_lobby_message};
