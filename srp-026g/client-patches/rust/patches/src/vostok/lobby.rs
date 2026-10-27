//! The lobby: its client (survarium::lobby_client) as the lobby menu's
//! callbacks reach it. At `lobby.replay_link_clicked` (sites_026g.tsv; the
//! lobby's video tutorial link, video_tutorial_link_clicked 0x28CDA0) EAX
//! is the lobby client its owner gave, whose +1E6144h is the link the
//! server's 9E sent (UTF-8, NUL-ended).

use core::marker::PhantomData;

use super::context::{Context, Reg};
use super::hook::Thread;
use super::object::{Object, read};
use crate::raw;

/// The link the server sent, read a byte at a time as the game's buffer
/// holds it (never past its NUL: a reader stops there).
pub struct ServerLink<'t> {
    client: Object,
    thread: PhantomData<&'t Thread>,
}

impl<'t> ServerLink<'t> {
    /// The link at `lobby.replay_link_clicked`: the lobby client in EAX.
    pub fn at_replay_link_clicked(ctx: &'t Context<'_>) -> Option<ServerLink<'t>> {
        Object::at(ctx.reg(Reg::Eax)).map(|client| ServerLink { client, thread: PhantomData })
    }

    /// The link's byte `index` (the caller reads no further than its NUL).
    pub fn byte(&self, index: usize) -> u8 {
        // SAFETY: the lobby client's link buffer, NUL-ended; a reader asks
        // for no byte past the NUL it met.
        unsafe { read(raw::lobby_client::replay_link(self.client.ptr()).add(index)) }
    }
}
