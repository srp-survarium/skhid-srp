//! The match link's UDP client (`vostok::network_core::udp_match_client`) at
//! `match_link.receive_failed`, on the network thread that runs its socket's
//! completions and every order of the match client (patch 132,
//! client_patch.md section 3.77).

use super::context::Context;
use super::world::read;
use crate::raw;

/// A failed receive, by what its socket is to the link.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReceiveFailure {
    /// udp_match_client::connect closed the socket and waits for its
    /// completions to connect again: the failure is that close.
    OfReplacedSocket,
    /// The link's socket as it is: the game's check decides.
    OfCurrentSocket,
}

impl ReceiveFailure {
    pub fn at(ctx: &Context<'_>) -> Option<ReceiveFailure> {
        use raw::site::match_link_receive_failed as site;
        // SAFETY: the site's EBX is the client whose handle_receive is
        // running; +1280h (udp_match_client::connect 0x3F2A8B and the two
        // completions, all on this thread) is the boost::function's manager
        // word, 0 when empty.
        let waiting = unsafe { read(site::client(ctx)?.connect_waiting()) };
        Some(if waiting != 0 { ReceiveFailure::OfReplacedSocket } else { ReceiveFailure::OfCurrentSocket })
    }
}
