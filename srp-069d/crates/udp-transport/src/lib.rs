//! The 0.69d client's reliable-UDP transport, server side. Only PROVEN items of the wire format are relied
//! on (see README).
//!
//! * [`ack`]: header codec (9-bit seq, R, ack0 + tail bytes).
//! * [`wire`]: message codec (single / multi format, multipart, low-level FA..FD).
//! * [`table`]: per-id classes (unique / ordered / plain) of the lobby connection.
//! * [`seq`]: 9-bit cyclic sequence numbers and the received-seq window.
//! * [`Transport`]: one peer, sans IO: `receive` datagrams, `send` messages, `poll` for datagrams to
//!   put on the wire (new messages, resends, acks, keepalives).

pub mod ack;
pub mod seq;
pub mod table;
mod transport;
pub mod wire;

pub use table::{class_of, Class, Direction};
pub use transport::{Config, Event, Role, SendError, Transport};
pub use wire::{Data, Datagram, Message, Part, MAX_DATAGRAM, PART};
