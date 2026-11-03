//! One synchronous dispatch owns the reader and its borrowed payload.
use super::{Context, Thread};
use super::world::read;
use crate::raw;

enum Payload<'t> {
    Readable { bytes: &'t [u8], end: *mut u8 },
    Invalid,
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
    pub fn is_valid(&self) -> bool {
        match &self.payload {
            Payload::Readable { bytes, .. } => srp_validate::messages::check(self.id, bytes).is_ok(),
            Payload::Invalid => false,
        }
    }

    pub fn leave(self) {
        if let Payload::Readable { end, .. } = self.payload {
            // SAFETY: end is the checked bound of this live reader.
            unsafe { self.reader.set_cursor(end); }
        }
        // SAFETY: the dispatch's own client. Its 0xEA handler leaves from
        // inside this dispatch as well. No borrowed bytes are read afterward.
        unsafe { raw::engine_close_current_match(Some(self.client), 2); }
    }
}
