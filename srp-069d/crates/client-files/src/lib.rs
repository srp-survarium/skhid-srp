//! client-files: readers of the 0.69d client's own files, so a server can build its tables from the operator's game
//! install instead of shipping them.
//!
//! - [`volume`]: one `resources.NN.db` volume (a file index followed by the file bytes);
//! - [`game`]: the game folder's volumes as one tree, the newest volume holding a path wins;
//! - [`options`]: the binary config format of `.options` files, `.profiles`, the language files and a few more;
//! - [`value`]: the ordered value those configs decode to;
//! - [`pyjson`]: a JSON writer whose output matches Python's `json.dumps` byte for byte (the layout of the server's
//!   tables, so tables written by other tools compare exactly);
//! - [`pe`]: reading a string table out of `survarium.exe` by virtual address.
//!
//! Nothing here writes anywhere: every reader takes a path or bytes and returns values.

pub mod cp1251;
pub mod game;
pub mod options;
pub mod pe;
pub mod pyjson;
pub mod value;
pub mod volume;

pub use game::GameFiles;
pub use options::{decode as decode_options, Encoding};
pub use value::{Dict, Value};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{path}: {source}")]
    Io { path: String, source: std::io::Error },
    #[error("{what}: {why}")]
    Format { what: String, why: String },
    #[error("{0} is not in the game's resources")]
    Missing(String),
}

impl Error {
    pub(crate) fn format(what: impl Into<String>, why: impl Into<String>) -> Error {
        Error::Format { what: what.into(), why: why.into() }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
