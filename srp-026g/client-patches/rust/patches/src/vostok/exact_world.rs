//! Patch 211's two sites (`exact_world`, `docs/notes/desync_capture.md`):
//!
//! - `world_hash.serialized`: `game_state_history_item::hash` (0x325150)
//!   right after it serialized the item (the call 0x324BE0 at 0x3251D7) into
//!   its buffer: the bytes are the world its CRC (and so a 0xDF report) is
//!   of, exactly what an upload of that item sends (client_patch.md §3.4);
//! - `upload_request.case`: `on_match_packet_received`'s case 0xF6
//!   (0x27A4F4), before the snapshot upload's request hook (0x27A505): the
//!   message's payload, and the connection to upload a world on.
//!
//! The bytes are read once here into slices; the patch keeps and sends them.

use super::context::Context;
use super::hook::Thread;
use super::packet_recovery::Connection;
use super::world::read;
use crate::raw;

/// A world the game just serialized to hash it: its time, then its body.
pub struct Hashed<'t> {
    pub time: u32,
    pub body: &'t [u8],
}

impl<'t> Hashed<'t> {
    pub fn at(ctx: &'t Context<'_>) -> Option<Hashed<'t>> {
        use raw::site::world_hash_serialized as site;
        // SAFETY: the site's ESI is the history item hash() was called for
        // and EBP its frame, both alive until it returns; only reads.
        let (item, frame) = unsafe { (site::item(ctx)?, site::frame(ctx)?) };
        // SAFETY: the item's time and the frame's mutable buffer {data,
        // size}, which the serialize just filled; plain words.
        let (time, data, size) = unsafe { (read(item.time()), frame.data(), read(frame.size())) };
        // hash()'s buffer is world_frame::MAX_BODY bytes (patch 98): a size
        // past it is no buffer of hash().
        let size = usize::try_from(size).ok().filter(|size| *size <= world_frame::MAX_BODY)?;
        if data.is_null() {
            return None;
        }
        // SAFETY: data..data+size are the bytes the serialize wrote into
        // hash()'s buffer, which hash() reads (the CRC) and frees after this
        // hook returns; nothing writes them meanwhile.
        let body = unsafe { core::slice::from_raw_parts(data as *const u8, size) };
        Some(Hashed { time, body })
    }
}

/// An upload request (0xF6) a live match's client takes.
pub struct Request<'t> {
    payload: &'t [u8],
    connection: Connection<'t>,
}

impl<'t> Request<'t> {
    /// The request, when the client is in the live match (its state 2, a
    /// match core: what the snapshot upload asks before it uploads);
    /// `None` otherwise, and the game goes on as it would.
    pub fn at(ctx: &'t Context<'_>) -> Option<Request<'t>> {
        use raw::site::upload_request_case as site;
        // SAFETY: the dispatcher's ESI (the network client) and EBX (the
        // message's reader), alive while the handler runs on this thread.
        let (client, reader) = unsafe { (site::client(ctx)?, site::reader(ctx)?) };
        // SAFETY: plain words of the client: the connection's state and the
        // live match wrapper.
        let live = unsafe { read(client.match_connection_state()) == 2 && client.pvp_match().is_some() };
        if !live {
            return None;
        }
        // SAFETY: a live reader {begin, cursor, size}, for the hook.
        let (begin, cursor, size) = unsafe { (reader.begin() as usize, reader.cursor() as usize, read(reader.size())) };
        let end = begin.checked_add(size as usize)?;
        if cursor < begin || cursor > end {
            return None;
        }
        // SAFETY: cursor..end lies inside the message's bytes, alive for the
        // hook; the handler reads none of them before its exit.
        let payload = unsafe { core::slice::from_raw_parts(cursor as *const u8, end - cursor) };
        let connection = Connection::of(client, ctx.thread())?;
        Some(Request { payload, connection })
    }

    pub fn payload(&self) -> &'t [u8] {
        self.payload
    }

    /// Uploads `body` as the world of `time`, every chunk or none (the
    /// packet pool has no room for them all): `false` then.
    pub fn upload(&self, time: u32, body: &[u8]) -> bool {
        self.connection.world(time, body)
    }

    pub fn thread(&self) -> &'t Thread {
        self.connection.thread()
    }
}
