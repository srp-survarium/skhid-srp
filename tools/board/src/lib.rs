//! Skhid status board: one JSON file rendered as one read-only HTML page.
//!
//! - `model`: the data file, its validation and dates;
//! - `render`: the page;
//! - `server`: routing, response headers and the reloading store.

pub mod model;
pub mod render;
pub mod server;
