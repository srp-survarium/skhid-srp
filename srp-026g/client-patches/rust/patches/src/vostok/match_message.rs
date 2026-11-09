//! One synchronous dispatch owns the reader and its borrowed payload.
use super::{Context, Thread};
use super::parse::Reader;
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

/// A late join's profile proposal (the server's `late_join::Prepare`): its
/// header (`FF 'L' 'J' 01`, the match, the serial, the place, the squad),
/// the native profile, then srp's block when it has an entry (as after the
/// options: `SRPX`, then `{tag, len, bytes}`). The header is patch 114's
/// check's; the profile the late join's C check's
/// (`late_join_validate_026g.c`, after the dispatch), which reads it to the
/// message's end: the block is taken off before ([`Message::end_at_profile`]).
pub struct Proposal<'t> {
    pub place: u8,
    /// Its player's name: the profile's first text, without a zero.
    pub name: &'t [u8],
    /// What follows the profile, to the message's end: srp's block, if any.
    pub block: &'t [u8],
}

/// Steps over a native profile (`player_profile::deserialize_static`
/// 0x1F1070, as the late join's C check reads it): the team, the name, the
/// clan, the protection, the kits and the items, each item's length by
/// whether the dictionary's entry is a stack (`stack`; none: an id it does
/// not have). The name.
fn step_over_profile<'a>(r: &mut Reader<'a>, stack: impl Fn(u16) -> Option<bool>) -> Option<&'a [u8]> {
    fn text<'a>(r: &mut Reader<'a>) -> Option<&'a [u8]> {
        let len = r.u8()?;
        r.take(usize::from(len))
    }
    fn each(r: &mut Reader<'_>, size: usize, count: u32) -> Option<()> {
        r.take(size.checked_mul(count as usize)?).map(drop)
    }
    r.u8()?; // the team
    let name = text(r)?;
    text(r)?; // the clan
    r.u32()?; // the protection
    let (kits, items) = (r.u8()?, r.u8()?);
    for _ in 0..kits {
        r.take(5)?; // its state, its id
        text(r)?;
        let slots = r.u32()?;
        each(r, 4, slots.count_ones())?; // a slot's item and count
        let modifiers = [r.u32()?, r.u32()?, r.u32()?, r.u32()?];
        each(r, 4, modifiers.iter().map(|mask| mask.count_ones()).sum())?;
    }
    for _ in 0..items {
        r.u32()?; // its instance
        let id = r.u16()?;
        if id == 0 {
            continue;
        }
        r.take(if stack(id)? { 4 } else { 12 })?;
        let modifications = r.u8()?;
        each(r, 5, u32::from(modifications))?;
        let upgrades = r.u8()?;
        each(r, 17, u32::from(upgrades))?;
    }
    Some(name)
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
    /// The late join's profile proposal it is (an 0xE4 of srp's dialect):
    /// none for any other message, or one whose profile the items
    /// dictionary cannot step over.
    pub fn proposal(&self) -> Option<Proposal<'t>> {
        let Payload::Readable { bytes, .. } = &self.payload else { return None };
        if self.id != srp_validate::messages::id::STATIC_INFO {
            return None;
        }
        let mut reader = Reader::new(bytes);
        reader.expect(&srp_validate::messages::LATE_JOIN_PREPARE)?;
        reader.take(8)?; // the match, the serial
        let place = reader.u8()?;
        reader.u8()?; // the squad
        // SAFETY: the dispatch's live client and its game, on the game's
        // thread; the dictionary is the game's, loaded once.
        let dictionary = unsafe { self.client.game()?.dictionary()? };
        let stack = |id: u16| {
            // SAFETY: item_by_id only looks the id up (0x1F0090); an entry
            // it finds is the dictionary's, its stack byte a plain value.
            unsafe { raw::engine_item_by_id(Some(dictionary), u32::from(id)).map(|item| read(item.stack()) != 0) }
        };
        let name = step_over_profile(&mut reader, stack)?;
        Some(Proposal { place, name, block: reader.remaining() })
    }

    /// Ends the message where its proposal's profile ends: srp's block after it
    /// read, the late join's code (`lj_prepare`, after the dispatch) gets the
    /// native profile alone, as its check wants it. Nothing for a proposal
    /// without a block, or one that is not this message's.
    pub fn end_at_profile(&mut self, proposal: Proposal<'t>) {
        let Payload::Readable { bytes, end } = &mut self.payload else { return };
        let cut = proposal.block.len();
        if cut == 0 || !core::ptr::eq(proposal.block.as_ptr_range().end, bytes.as_ptr_range().end) {
            return;
        }
        // SAFETY: the dispatch's live reader; its size only shrinks, by bytes
        // of its own (the block is the payload's tail, and the payload ends at
        // begin + size).
        unsafe { self.reader.size().write_volatile(read(self.reader.size()).saturating_sub(cut as u32)) };
        *bytes = &bytes[..bytes.len() - cut];
        *end = end.wrapping_sub(cut);
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
            match (&connection, generation) {
                (Some(connection), Some(generation)) => connection.rejected(generation),
                (Some(connection), None) => connection.unreadable(),
                (None, _) => {}
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A profile of two kits and three items (a stack, one that is not
    /// with a modification and an upgrade, an empty entry), as the server's
    /// `static_info::write_profile` puts it.
    fn profile() -> std::vec::Vec<u8> {
        let mut bytes = b"\x01\x04late\x02SR".to_vec();
        bytes.extend(10u32.to_le_bytes());
        bytes.extend([2, 3]);
        for kit in 0..2u32 {
            bytes.push(0);
            bytes.extend((kit + 1).to_le_bytes());
            bytes.extend(b"\x01K");
            bytes.extend(3u32.to_le_bytes()); // slots 0 and 1
            bytes.extend([0, 0, 1, 0, 1, 0, 1, 0]);
            for mask in [8u32, 0, 1, 0] {
                bytes.extend(mask.to_le_bytes());
            }
            bytes.extend([0; 8]); // modifiers 67 and 0
        }
        bytes.extend(1u32.to_le_bytes());
        bytes.extend(101u16.to_le_bytes());
        bytes.extend([0; 4]); // a stack's count
        bytes.extend([0, 0]);
        bytes.extend(2u32.to_le_bytes());
        bytes.extend(102u16.to_le_bytes());
        bytes.extend([0; 12]); // condition, repair and reroll times
        bytes.extend([1, 0, 0, 0, 0, 0]);
        bytes.extend([1]);
        bytes.extend([0; 17]);
        bytes.extend(3u32.to_le_bytes());
        bytes.extend(0u16.to_le_bytes());
        bytes
    }

    fn stack(id: u16) -> Option<bool> {
        match id {
            101 => Some(true),
            102 => Some(false),
            _ => None,
        }
    }

    #[test]
    fn a_profile_is_stepped_over_to_what_follows_it() {
        for after in [&b""[..], b"SRPX\x03\x01\x02"] {
            let bytes = [&profile()[..], after].concat();
            let mut reader = Reader::new(&bytes);
            assert_eq!(step_over_profile(&mut reader, stack), Some(&b"late"[..]));
            assert_eq!(reader.remaining(), after);
        }
        let whole = profile();
        for end in 0..whole.len() {
            assert_eq!(step_over_profile(&mut Reader::new(&whole[..end]), stack), None, "{end}");
        }
        // An item the dictionary does not have: its length is not known.
        assert_eq!(step_over_profile(&mut Reader::new(&whole), |id| (id != 102).then_some(true)), None);
    }
}
