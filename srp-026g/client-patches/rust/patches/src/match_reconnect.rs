//! Patch 132: a receive that failed on the socket a new connect has already
//! closed is that connect's own doing, never the link's loss.
//!
//! udp_match_client::connect (0x3F2990) closes the old socket and, while a
//! receive or a send of it is outstanding, leaves the new connect to the last
//! completion. The game's handle_receive runs that connect only when no send
//! is outstanding; with one (issued on the closed socket, so it completes with
//! WSAEBADF) it took the aborted receive (995) for a lost link: the game was
//! told the match ended (reason 1) as the new connection was being made.
//! client_patch.md section 3.77.

#![forbid(unsafe_code)]

use crate::vostok::match_link::ReceiveFailure;
use crate::vostok::{Context, Flow, MatchReconnectOnReceiveFailedExit};

/// handle_receive's error path, after its log line: an error of the replaced
/// socket returns as a receive on a dead link does. The send still
/// outstanding runs the connect when it completes.
pub fn on_receive_failed(ctx: &mut Context<'_>) -> Flow<MatchReconnectOnReceiveFailedExit> {
    match ReceiveFailure::at(ctx) {
        Some(ReceiveFailure::OfReplacedSocket) => Flow::Exit(MatchReconnectOnReceiveFailedExit::Done),
        Some(ReceiveFailure::OfCurrentSocket) | None => Flow::Continue,
    }
}
