//! The 0.26g client's reliable-UDP transport, server side: shared by the
//! lobby server and the match server. The spec is
//! `docs/notes/transport.md`; section numbers in the comments
//! (§2.2, §4.1 …) point there, and hex addresses are in the 0.26g client.
//!
//! - [`Transport`]: one per client. It owns no socket: a server keeps one
//!   UDP socket and a map of peers, feeds each datagram to its peer's
//!   `receive`, and sends what `poll` returns.
//! - [`Table`]: which connection it is. The match and the lobby run on the
//!   same transport code in the client, with different per-id tables.
//! - [`PacketLog`]: the opt-in raw datagram log (`SRP_PACKET_LOG=<dir>`).
//! - [`wire`], [`ack`], [`seq`]: the formats, usable on their own (the
//!   parser reads both directions).
//!
//! ```no_run
//! use std::collections::HashMap;
//! use std::net::{SocketAddr, UdpSocket};
//! use std::time::Instant;
//! use udp_transport::{Event, PacketLog, Table, Transport};
//!
//! let socket = UdpSocket::bind("0.0.0.0:1236").unwrap();
//! let log = PacketLog::from_env();
//! let mut peers: HashMap<SocketAddr, Transport> = HashMap::new();
//! let mut buffer = [0; 2048];
//! loop {
//!     let (len, from) = socket.recv_from(&mut buffer).unwrap();
//!     let now = Instant::now();
//!     let peer = peers.entry(from).or_insert_with(|| {
//!         let mut transport = Transport::new(Table::Match, now);
//!         if let Some(Ok(file)) = log.as_ref().map(|log| log.connection(Table::Match, from)) {
//!             transport.set_log(file);
//!         }
//!         transport
//!     });
//!     for event in peer.receive(&buffer[..len], now) {
//!         if let Event::Message { id: 0xD8, .. } = event {
//!             peer.send(0xE3, &[0xD4, 0x04, 0xD4, 0x04]).unwrap();
//!         }
//!     }
//!     for datagram in peer.poll(now) {
//!         socket.send_to(&datagram, from).unwrap();
//!     }
//! }
//! ```

pub mod ack;
pub mod packet_log;
pub mod seq;
pub mod table;
mod transport;
pub mod wire;

#[cfg(test)]
mod tests;

pub use packet_log::{ConnectionLog, PacketLog, Redact};
pub use table::{Class, Direction, Length, Lifetime, Lookup, MessageInfo, Table};
pub use transport::{Config, Event, SendError, State, Transport};
pub use wire::MAX_DATAGRAM;
