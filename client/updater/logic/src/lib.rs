//! What the Skhid updater does that needs no Windows, so that it is tested
//! on any host: the checksum, the ZIP reader, the server's answer, the file
//! that remembers what is installed, and the launcher's pipe messages.
#![cfg_attr(not(test), no_std)]

extern crate alloc;

pub mod args;
pub mod bencode;
pub mod checked;
pub mod download;
pub mod file_error;
pub mod game;
pub mod inflate;
pub mod local_replay;
pub mod manifest;
pub mod network;
pub mod patches;
pub mod pipe;
pub mod problem;
pub mod rendezvous;
pub mod replay;
pub mod resources;
pub mod revision;
pub mod sha1;
pub mod sha256;
pub mod signed;
pub mod swarm;
pub mod torrent;
pub mod tracker;
pub mod url;
pub mod wire;
pub mod zip;
