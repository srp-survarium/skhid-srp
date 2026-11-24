//! [body physics S5a] The one model of the 0.26g client's body lives in `mapdata::motion` (the builder calls it); this crate
//! re-exports it, so its users name it as before. No definition here.

pub use mapdata::motion::*;
