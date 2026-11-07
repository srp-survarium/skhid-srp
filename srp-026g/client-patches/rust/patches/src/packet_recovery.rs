//! Receipts describe this client's load, never an accusation against a peer.
#![forbid(unsafe_code)]
use crate::vostok::{Context, Flow, NoExit};
use crate::vostok::packet_recovery::Connection;

pub fn on_loaded(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(connection) = Connection::at_loaded(ctx) { connection.loaded(); }
    Flow::Continue
}
pub fn on_close(ctx: &mut Context<'_>) -> Flow<NoExit> {
    Connection::closing(ctx);
    Flow::Continue
}
/// A receipt the pool had no room for goes on the next frame with room.
pub fn on_frame(ctx: &mut Context<'_>) -> Flow<NoExit> {
    if let Some(connection) = Connection::at_game_tick(ctx) { connection.send_waiting(); }
    Flow::Continue
}
