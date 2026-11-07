//! One synchronous dispatch owns the reader and its borrowed payload.
use super::{Context, Thread};
use super::world::read;
use crate::raw;

enum Payload<'t> {
    Readable { bytes: &'t [u8], end: *mut u8 },
    Invalid,
}

/// Why a message is discarded.
#[derive(Clone, Copy, Debug)]
pub enum Fault {
    /// The native reader's cursor is outside its buffer.
    Reader,
    Refused(srp_validate::Refusal),
}

pub struct Message<'t> {
    id: u8,
    client: raw::NetworkClient,
    reader: raw::BufferReader,
    payload: Payload<'t>,
    _thread: &'t Thread,
}

impl<'t> Message<'t> {
    pub fn at_dispatch(ctx: &'t Context<'_>) -> Option<Self> {
        use raw::site::match_dispatch_message as site;
        // SAFETY: these are the dispatch's native client and reader, and the normalized id, alive on the game's thread for the duration of the hook.
        let (client, reader, id) = unsafe {
            (site::client(ctx)?, site::reader(ctx)?, site::id(ctx) as u8)
        };
        // SAFETY: the reader owns begin..begin+size. Read bounds once; a
        // cursor outside them is never converted to a Rust slice.
        let (begin, cursor, size) = unsafe {
            (reader.begin() as usize, reader.cursor() as usize, read(reader.size()) as usize)
        };
        let payload = match begin.checked_add(size) {
            Some(end) if cursor != 0 && cursor >= begin && cursor <= end => {
                // SAFETY: the borrowed range lies inside the live reader's
                // allocation; no native handler runs until the hook returns.
                let bytes = unsafe { core::slice::from_raw_parts(cursor as *const u8, end-cursor) };
                Payload::Readable { bytes, end: end as *mut u8 }
            }
            _ => Payload::Invalid,
        };
        Some(Self { id, client, reader, payload, _thread: ctx.thread() })
    }

    pub fn id(&self) -> u8 { self.id }
    /// Its payload's length; none when the reader's bounds do not hold.
    pub fn payload_len(&self) -> Option<usize> {
        match &self.payload {
            Payload::Readable { bytes, .. } => Some(bytes.len()),
            Payload::Invalid => None,
        }
    }
    /// Why its handler may not read it: `None` when it may.
    pub fn fault(&self) -> Option<Fault> {
        match &self.payload {
            Payload::Readable { bytes, .. } => srp_validate::messages::check(self.id, bytes).err().map(Fault::Refused),
            Payload::Invalid => Some(Fault::Reader),
        }
    }

    /// Consume only the checked reader range. Rejection is not a user leave.
    /// A world envelope refusal requests recovery; ordinary bad messages are
    /// dropped. The sender is policed by the server before relay.
    pub fn discard(self) {
        let connection = super::packet_recovery::Connection::of(self.client, self._thread);
        if self.id == srp_validate::messages::id::SNAPSHOT {
            let generation = match &self.payload {
                Payload::Readable { bytes, .. } => bytes.first().copied(),
                Payload::Invalid => None,
            };
            if let Some(connection) = &connection { connection.rejected(generation); }
        }
        // A late join's profile proposal (an 0xE4 to a game in play) is no
        // handshake: no handshake is sent again for it.
        let proposal = matches!(&self.payload, Payload::Readable { bytes, .. }
            if bytes.starts_with(&srp_validate::messages::LATE_JOIN_PREPARE));
        if matches!(self.id, srp_validate::messages::id::STATIC_INFO | srp_validate::messages::id::PORTS)
            && !proposal
            && let Some(connection) = &connection
        { connection.options_rejected(); }
        if let Payload::Readable { end, .. } = self.payload {
            // SAFETY: end was checked before borrowing the reader's bytes.
            unsafe { self.reader.set_cursor(end); }
        }
    }
}
