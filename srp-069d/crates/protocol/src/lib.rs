//! protocol (0.69d): message payload encode/decode for the lobby connection.
//!
//! Scope: the bytes after the `id x` header of a message (the datagram transport is `udp-transport`).
//! Server to client messages are in [`s2c`], client to server requests in [`c2s`]; each message is a plain
//! struct with `to_bytes` / `from_bytes` (traits [`Encode`] / [`Decode`]). The tests compare against bytes
//! that worked with the real client (`tests/vectors/*.json`).
//!
//! Evidence tags in the doc comments: PROVEN = read in the client code at the quoted address (or seen working in the
//! real client), LIKELY = layout proven but meaning inferred, or not yet seen working against the client.

pub mod binconfig;
pub mod c2s;
pub mod ids;
pub mod items;
pub mod lzma2;
pub mod s2c;
pub mod wire;

pub use ids::MsgClass;
pub use wire::{Decode, Encode, Reader, Result, Writer};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("truncated message: {need} more byte(s) needed at offset {at}")]
    Truncated { at: usize, need: usize },
    #[error("{0} trailing byte(s) after the message")]
    Trailing(usize),
    #[error("limit exceeded: {0}")]
    Limit(&'static str),
    #[error("invalid value: {0}")]
    Invalid(&'static str),
    #[error("unsupported layout: {0}")]
    Unsupported(&'static str),
    #[error("lzma2: {0}")]
    Lzma(String),
}
