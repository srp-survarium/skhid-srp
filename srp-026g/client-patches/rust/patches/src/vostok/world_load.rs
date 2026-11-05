//! A world the game is about to load (0xE8, patch 87): the 0xE8 handler
//! `network_client::process_dynamic_match_info` (0x27BFF0) loads it with
//! `game_world_core::deserialize` (0x2D0B70), which reads it with no check of
//! its own, at two sites (sites_026g.tsv):
//! - `world_load.unpacked` (before the load at 0x27C0D9): the world the
//!   handler unpacked into its frame's mutable buffer (EBP-24h); the
//!   packet's reader was read to its end already (0x27C0B7);
//! - `world_load.raw` (before the load at 0x27C111): the world the rest of
//!   the packet's reader (ESI).
//!
//! Either way the bytes are the world's time (u32), then its body, read
//! once here into a slice; the patch parses them (`world_frame`). A world
//! refused is never loaded: [`WorldLoad::leave`] leaves the match as the
//! leave button does, and the site's exit `refused` is the handler's
//! epilogue (0x27C1E0), past the post-load steps (world_frame_check.md).

use super::context::Context;
use super::hook::Thread;
use super::world::{read, NetworkClient};
use crate::raw;

/// `close_current_match`'s reason 2, "disconnected_by_initiator" (the
/// leave button's): 0xE0 to the match server, `31 .. 01` to the lobby, the
/// world ended (match.md section 25).
const LEAVE_BY_INITIATOR: u32 = 2;

/// Where the world's bytes are, and so what leaving does to them.
enum Source {
    /// In the handler's buffer; the packet was read to its end already.
    Unpacked,
    /// The rest of the packet's reader: read to its end on a leave, as the
    /// handler's stale-world path does (0x27C02E).
    Packet(raw::BufferReader),
}

/// The world at one of the two loads.
pub struct WorldLoad<'t> {
    client: raw::NetworkClient,
    state: &'t [u8],
    source: Source,
    thread: &'t Thread,
}

impl<'t> WorldLoad<'t> {
    /// At `world_load.unpacked`.
    pub fn at_unpacked(ctx: &'t Context<'_>) -> Option<WorldLoad<'t>> {
        use raw::site::world_load_unpacked as site;
        // SAFETY: the context is this site's: the network client and the
        // handler's frame, whose buffer holds the unpacked world (the 0x8000
        // bytes of its stack), alive while it runs on the game's thread.
        let (client, buffer) = unsafe { (site::client(ctx)?, site::frame(ctx)?.unpacked()) };
        // SAFETY: the buffer is alive for the hook, `size` bytes from its
        // begin (resize at 0x27C0A8 set it within its capacity).
        let state = unsafe { bytes(buffer.begin(), read(buffer.size()) as usize) };
        Some(WorldLoad { client, state, source: Source::Unpacked, thread: ctx.thread() })
    }

    /// At `world_load.raw`.
    pub fn at_raw(ctx: &'t Context<'_>) -> Option<WorldLoad<'t>> {
        use raw::site::world_load_raw as site;
        // SAFETY: the context is this site's: the network client and the
        // packet's reader, alive while the handler runs on the game's thread.
        let (client, reader) = unsafe { (site::client(ctx)?, site::reader(ctx)?) };
        // SAFETY: a live reader {begin, cursor, size}, for the hook.
        let (begin, cursor, size) = unsafe { (reader.begin() as usize, reader.cursor() as usize, read(reader.size())) };
        let end = begin.checked_add(size as usize)?;
        if cursor < begin || cursor > end {
            return None;
        }
        // SAFETY: begin..end are the packet's bytes, alive for the hook, and
        // cursor..end lies inside them.
        let state = unsafe { bytes(cursor as *const u8, end - cursor) };
        Some(WorldLoad { client, state, source: Source::Packet(reader), thread: ctx.thread() })
    }

    /// The world: its time (u32), then its body.
    pub fn state(&self) -> &'t [u8] {
        self.state
    }

    /// The handler's network client (its places' players).
    pub fn network_client(&self) -> NetworkClient<'t> {
        NetworkClient::of(self.client, self.thread)
    }

    /// The hook's thread.
    pub fn thread(&self) -> &'t Thread {
        self.thread
    }

    /// Leaves the match instead of loading the world: the packet read to its
    /// end, then `close_current_match(2)`. The hook's exit `refused` next.
    pub fn leave(self) {
        if let Source::Packet(reader) = self.source {
            // SAFETY: the packet's live reader, for the hook; its end is
            // inside its bytes.
            unsafe {
                let end = (reader.begin() as usize).wrapping_add(read(reader.size()) as usize);
                reader.set_cursor(end as *mut u8);
            }
        }
        // SAFETY: the handler's own network client, on the game's thread;
        // the 0xEA handler makes the same call from inside the dispatch
        // (0x27C720..0x27C76F).
        unsafe { raw::engine_close_current_match(Some(self.client), LEAVE_BY_INITIATOR) }
    }
}

/// The `len` bytes at `at`; none for a null pointer.
///
/// # Safety
/// `len` bytes from `at` are readable and not written for the hook.
unsafe fn bytes<'t>(at: *const u8, len: usize) -> &'t [u8] {
    if at.is_null() {
        return &[];
    }
    unsafe { core::slice::from_raw_parts(at, len) }
}
