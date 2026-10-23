//! The datagram and message formats (transport.md §2.1, §3.1, §3.2), and a
//! parser that follows the client's `try_parse_single_message` (0x3FA240)
//! rule for rule. The server parses client datagrams with it, and the packet
//! log decodes both directions with it.
//!
//! ```text
//! datagram:   seq rseq ack0 <messages> [tail ack bytes]
//! normal:     id [x] [len] payload
//! multipart:  FF id x part [count] [len] data        (count only in part 0)
//! low-level:  id (0xFA..=0xFE), nothing after it
//! ```
//!
//! - `x` is one byte: the order id of an ordered id, else the unique id of a
//!   unique id; plain ids have none.
//! - `len` exists only for a variable-length id in a multi datagram: `u8`
//!   up to 254, else `FF u16`. It counts the payload only. A fixed-length id
//!   never has one, and in a single datagram it must fill the message area
//!   exactly.
//! - A single datagram holds one message; a multi one up to 252.

use crate::ack::{self, Acks};
use crate::table::{Class, Direction, Length, Lookup, Table, player_inputs_len};

/// The client's receive buffer (`start_receiving`, 0x3F25D9): header,
/// messages and tail ack bytes together.
pub const MAX_DATAGRAM: usize = 0x1FC;
/// `seq rseq ack0`.
pub const HEADER: usize = 3;
/// The client parses into a `fixed_vector<message_data, 252>`; one more is a
/// fatal assert.
pub const MAX_MESSAGES: usize = 252;
/// The id byte that starts a part of a split message.
pub const MULTIPART: u8 = 0xFF;

/// The match's client→server leave, which a patched client also sends with
/// a payload: a chunk of its world (client_patch.md §3.4).
pub const UPLOAD_CARRIER: u8 = 0xE0;
/// The largest world a patched client uploads: 0xFF00 bytes since patch 60
/// (`client_patch.md` §3.54; `survarium::v026g::match_msgs::upload::MAX_TOTAL`,
/// which a test of that crate compares with this).
pub const UPLOAD_MAX_TOTAL: usize = 0xFF00;

/// The low-level ids (transport.md §3.4): one byte, last in the datagram.
pub mod low_level {
    /// "I am leaving": the peer answers `CONFIRM` and is gone 500 ms later.
    pub const INITIATE_DISCONNECTION: u8 = 0xFA;
    /// The answer to `INITIATE`.
    pub const CONFIRM_DISCONNECTION: u8 = 0xFB;
    /// The keep-alive and pure ack.
    pub const CONTINUOUS_FLOW: u8 = 0xFC;
    /// The peer disconnects at once, in any state.
    pub const INSTANT_DISCONNECT: u8 = 0xFD;
    /// The client's jump table entry is garbage: never send it.
    pub const FORBIDDEN: u8 = 0xFE;
}

/// Where a part belongs in its split message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Part {
    pub index: u8,
    /// The number of parts; only part 0 carries it.
    pub count: Option<u8>,
}

/// One message of a datagram.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Message<'a> {
    pub id: u8,
    /// One of the ids 0xFA..=0xFE; the other fields are then empty.
    pub low_level: bool,
    pub class: Class,
    /// The unique id or order id, by `class`.
    pub x: Option<u8>,
    pub part: Option<Part>,
    pub payload: &'a [u8],
    /// Where the payload starts in the datagram.
    pub offset: usize,
}

/// A parsed datagram.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Datagram<'a> {
    /// The sender's seq of this datagram.
    pub seq: u8,
    /// The newest seq the sender has received.
    pub rseq: u8,
    pub acks: Acks,
    pub messages: Vec<Message<'a>>,
}

/// Why the client's parser (and so this one) refuses a datagram. The whole
/// datagram is then dropped: no acks are processed and its seq is not acked
/// (transport.md §2.3 steps 2 and 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseError {
    /// Shorter than 3 bytes, or a broken ack tail.
    Header,
    /// No message at all: a header-only datagram.
    EmptyMessageArea,
    /// A message runs past the end.
    Truncated,
    /// An id outside the table's range ("invalid message type").
    InvalidId(u8),
    /// An id whose table entry is null: the client jumps to address 0.
    NullId(u8),
    /// A low-level id that is not the last byte ("non-empty low level
    /// packet").
    LowLevelNotLast,
    /// A fixed-length id in a single datagram whose payload has another
    /// size ("single message packet, but extra space is found").
    FixedMismatch { id: u8, expected: usize, got: usize },
    /// The length getter reported an error (too short for the id).
    BadLength(u8),
    /// `FF u16` used for a length below 255.
    LongPrefixForShort,
    /// More than 252 messages.
    TooManyMessages,
    /// A part of a plain id: the sender writes an x byte the reader does
    /// not read, so it cannot be parsed.
    PlainMultipart(u8),
}

/// The size of the length prefix of a variable payload in a multi datagram.
pub fn prefix_len(payload: usize) -> usize {
    if payload <= 0xFE { 1 } else { 3 }
}

/// Append the length prefix of a variable payload.
pub fn push_prefix(buffer: &mut Vec<u8>, payload: usize) {
    if payload <= 0xFE {
        buffer.push(payload as u8);
    } else {
        buffer.push(0xFF);
        buffer.extend((payload as u16).to_le_bytes());
    }
}

/// Parse a whole datagram sent in `direction` on a `table` connection.
/// `mask_bytes` is the ED/EE player mask size (0 when unknown or not a
/// match).
pub fn parse(
    table: Table,
    direction: Direction,
    datagram: &[u8],
    mask_bytes: usize,
) -> Result<Datagram<'_>, ParseError> {
    let acks = ack::decode(datagram).ok_or(ParseError::Header)?;
    let end = datagram.len() - acks.tail;
    if end <= HEADER {
        return Err(ParseError::EmptyMessageArea);
    }
    let mut messages = Vec::new();
    let mut at = HEADER;
    loop {
        if messages.len() == MAX_MESSAGES {
            return Err(ParseError::TooManyMessages);
        }
        let (message, next) = parse_message(
            table,
            direction,
            &datagram[..end],
            at,
            acks.multi,
            mask_bytes,
        )?;
        messages.push(message);
        at = next;
        if at == end {
            break;
        }
        // A single datagram's message always takes the whole area.
        debug_assert!(acks.multi);
    }
    Ok(Datagram {
        seq: datagram[0],
        rseq: datagram[1],
        acks,
        messages,
    })
}

/// One message starting at `at`; `area` ends where the messages end.
fn parse_message(
    table: Table,
    direction: Direction,
    area: &[u8],
    mut at: usize,
    multi: bool,
    mask_bytes: usize,
) -> Result<(Message<'_>, usize), ParseError> {
    let end = area.len();
    let take = |at: &mut usize| -> Result<u8, ParseError> {
        let byte = *area.get(*at).ok_or(ParseError::Truncated)?;
        *at += 1;
        Ok(byte)
    };
    let mut id = take(&mut at)?;
    let multipart = id == MULTIPART;
    if multipart {
        if end - at < 2 {
            return Err(ParseError::Truncated);
        }
        id = take(&mut at)?;
    } else if (low_level::INITIATE_DISCONNECTION..=low_level::FORBIDDEN).contains(&id) {
        if at != end {
            return Err(ParseError::LowLevelNotLast);
        }
        let message = Message {
            id,
            low_level: true,
            class: Class::Plain,
            x: None,
            part: None,
            payload: &area[at..at],
            offset: at,
        };
        return Ok((message, at));
    }
    let info = match table.lookup(direction, id) {
        Lookup::Info(info) => info,
        Lookup::OutOfRange => return Err(ParseError::InvalidId(id)),
        Lookup::Null => return Err(ParseError::NullId(id)),
    };
    let x = match info.class {
        Class::Plain => None,
        Class::Unique | Class::Ordered => Some(take(&mut at)?),
    };
    let part = if multipart {
        if info.class == Class::Plain {
            return Err(ParseError::PlainMultipart(id));
        }
        let index = take(&mut at)?;
        let count = if index == 0 {
            Some(take(&mut at)?)
        } else {
            None
        };
        Some(Part { index, count })
    } else {
        None
    };
    let first = part.is_none_or(|part| part.index == 0);
    let remaining = end - at;
    let fixed = |len: usize| {
        if multi {
            if len > remaining {
                return Err(ParseError::Truncated);
            }
        } else if len != remaining {
            return Err(ParseError::FixedMismatch {
                id,
                expected: len,
                got: remaining,
            });
        }
        Ok(len)
    };
    // Patched clients use E0 for snapshots and resource ACKs. Single messages
    // consume the whole area; multi messages need the extension's own length.
    // An E0 without either extension prefix remains an empty leave.
    let carrier = table == Table::Match
        && direction == Direction::ToServer
        && id == UPLOAD_CARRIER
        && !multipart;
    let joined_chunk = if carrier && multi {
        carrier_len(&area[at..end])
    } else {
        None
    };
    let chunk = carrier && !multi;
    let len = match info.length {
        Length::Fixed(_) if chunk => remaining,
        Length::Fixed(_) if joined_chunk.is_some() => joined_chunk.unwrap_or(0),
        Length::Fixed(len) => fixed(len)?,
        Length::PlayerInputs { head } => {
            let len = player_inputs_len(head, mask_bytes, &area[at..])
                .ok_or(ParseError::BadLength(id))?;
            fixed(len)?
        }
        Length::Variable {
            first_min,
            part_min,
        } => {
            if remaining < if first { first_min } else { part_min } {
                return Err(ParseError::BadLength(id));
            }
            if multi {
                let mut len = usize::from(take(&mut at)?);
                if len == 0xFF {
                    len = usize::from(take(&mut at)?);
                    len |= usize::from(take(&mut at)?) << 8;
                    if len <= 0xFE {
                        return Err(ParseError::LongPrefixForShort);
                    }
                }
                if len > end - at {
                    return Err(ParseError::Truncated);
                }
                len
            } else {
                remaining
            }
        }
    };
    let message = Message {
        id,
        low_level: false,
        class: info.class,
        x,
        part,
        payload: &area[at..at + len],
        offset: at,
    };
    Ok((message, at + len))
}

/// Length of a resource ACK or whole snapshot chunk. Snapshot header:
/// `u32 item_time` (a tick), `u16 offset` (a
/// multiple of 480), `u16 total` (over the offset, at most
/// [`UPLOAD_MAX_TOTAL`]), then
/// `min(480, total - offset)` bytes (client_patch.md §3.4).
fn carrier_len(bytes: &[u8]) -> Option<usize> {
    let head = bytes.get(..8)?;
    // The experimental resource ACK has an impossible snapshot prefix.
    // Native datagrams can retain the multi flag even with joining disabled.
    if head == [0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0, 0] {
        return (bytes.len() >= 20).then_some(20);
    }
    let time = u32::from_le_bytes([head[0], head[1], head[2], head[3]]);
    let offset = usize::from(u16::from_le_bytes([head[4], head[5]]));
    let total = usize::from(u16::from_le_bytes([head[6], head[7]]));
    let len = 8 + (total.checked_sub(offset)?).min(480);
    (time.is_multiple_of(50)
        && offset.is_multiple_of(480)
        && offset < total
        && total <= UPLOAD_MAX_TOTAL
        && bytes.len() >= len)
        .then_some(len)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_match(direction: Direction, datagram: &[u8]) -> Result<Datagram<'_>, ParseError> {
        parse(Table::Match, direction, datagram, 3)
    }

    #[test]
    fn the_first_client_datagram_is_the_connect() {
        // transport.md §7: `00 FF 00 D8 <u32 session> 00`.
        let datagram = [0x00, 0xFF, 0x00, 0xD8, 0x78, 0x56, 0x34, 0x12, 0x00];
        let parsed = parse_match(Direction::ToServer, &datagram).unwrap();
        assert_eq!((parsed.seq, parsed.rseq), (0, 0xFF));
        assert!(!parsed.acks.multi);
        assert_eq!(parsed.acks.bits, 1 << 127);
        assert_eq!(parsed.messages.len(), 1);
        let connect = parsed.messages[0];
        assert_eq!(connect.id, 0xD8);
        assert_eq!(connect.x, None);
        assert_eq!(connect.payload, [0x78, 0x56, 0x34, 0x12, 0x00]);
        assert_eq!(connect.offset, 4);
    }

    #[test]
    fn resource_ack_in_a_multi_datagram_preserves_following_messages_and_tail_acks() {
        let ack = [
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0, 0, 9, 0, 0, 0x80, 1, 0, 0, 0, 2, 0, 1, 0,
        ];
        for before in [false, true] {
            for after in [false, true] {
                let mut bytes = vec![7, 3, 0x81];
                if before {
                    bytes.push(0xe1);
                }
                bytes.push(0xe0);
                bytes.extend(ack);
                if after {
                    bytes.push(0xe1);
                }
                bytes.push(1);
                let parsed = parse_match(Direction::ToServer, &bytes).unwrap();
                assert_eq!(
                    parsed.messages.len(),
                    1 + usize::from(before) + usize::from(after)
                );
                assert_eq!(parsed.messages[usize::from(before)].payload, ack);
            }
        }
        for len in 1..ack.len() {
            let mut bytes = vec![7, 3, 1, 0xe0];
            bytes.extend(&ack[..len]);
            assert!(parse_match(Direction::ToServer, &bytes).is_err());
        }
        // Native client: resource ACK followed by a world-hash report.
        let captured = [
            0x72, 0x84, 0x7f, 0xe0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0, 0, 0x64, 0xbb, 0xf2,
            0xc0, 1, 0, 0, 0, 2, 0, 1, 0, 0xdf, 0x25, 0x16, 0x14, 0xe1, 0x15,
        ];
        let parsed = parse_match(Direction::ToServer, &captured).unwrap();
        assert_eq!(parsed.messages.len(), 2);
        assert_eq!(parsed.messages[0].payload, &captured[4..24]);
        assert_eq!(parsed.messages[1].id, 0xdf);
        assert_eq!(parsed.messages[1].payload, &captured[25..]);
    }

    #[test]
    fn a_leave_with_a_payload_is_a_snapshot_chunk_alone_in_its_datagram() {
        // client_patch.md §3.4: `seq rseq ack0 E0 <payload> [tail acks]`.
        let mut datagram = vec![7, 3, 0x80, 0xE0];
        datagram.extend([0x32, 0, 0, 0, 0, 0, 2, 0, 0xAA, 0xBB]);
        datagram.push(0x00); // one tail ack byte
        let parsed = parse_match(Direction::ToServer, &datagram).unwrap();
        assert_eq!(parsed.messages.len(), 1);
        assert_eq!(parsed.messages[0].id, 0xE0);
        assert_eq!(
            parsed.messages[0].payload,
            [0x32, 0, 0, 0, 0, 0, 2, 0, 0xAA, 0xBB]
        );
        // Joined with the ready message (0xDA) and a keep-alive in a multi
        // datagram: the chunk's header gives its length.
        let mut datagram = vec![7, 3, 0x01, 0xE0];
        datagram.extend([0x32, 0, 0, 0, 0, 0, 2, 0, 0xAA, 0xBB]);
        datagram.extend([0xDA, 0xE1]);
        let parsed = parse_match(Direction::ToServer, &datagram).unwrap();
        let ids: Vec<u8> = parsed.messages.iter().map(|message| message.id).collect();
        assert_eq!(ids, [0xE0, 0xDA, 0xE1]);
        assert_eq!(
            parsed.messages[0].payload,
            [0x32, 0, 0, 0, 0, 0, 2, 0, 0xAA, 0xBB]
        );
        // Behind another message too; a last chunk cut short is no chunk
        // (and then no datagram: 0xAA is no id).
        let mut datagram = vec![7, 3, 0x01, 0xDA, 0xE0];
        datagram.extend([0x32, 0, 0, 0, 0, 0, 2, 0, 0xAA, 0xBB]);
        let parsed = parse_match(Direction::ToServer, &datagram).unwrap();
        assert_eq!(parsed.messages[1].payload.len(), 10);
        datagram.pop();
        assert!(parse_match(Direction::ToServer, &datagram).is_err());
        // The last chunk of the largest world (patch 60: 0xFF00 bytes, at
        // 0xFD20) joined; one of a larger total is no chunk.
        let mut datagram = vec![7, 3, 0x01, 0xE0];
        datagram.extend([0x32, 0, 0, 0, 0x20, 0xFD, 0x00, 0xFF]);
        datagram.extend([0x5A; 480]);
        datagram.push(0xE1);
        let parsed = parse_match(Direction::ToServer, &datagram).unwrap();
        assert_eq!(parsed.messages[0].payload.len(), 488);
        assert_eq!(parsed.messages[1].id, 0xE1);
        datagram[11] = 0xFF;
        datagram[10] = 0x01;
        assert!(parse_match(Direction::ToServer, &datagram).is_err());
        // The real leave is empty, single or in a multi datagram.
        let parsed = parse_match(Direction::ToServer, &[8, 3, 0x00, 0xE0]).unwrap();
        assert!(parsed.messages[0].payload.is_empty());
        let parsed = parse_match(Direction::ToServer, &[9, 3, 0x01, 0xE0, 0xE1]).unwrap();
        assert_eq!(parsed.messages.len(), 2);
        assert!(parsed.messages[0].payload.is_empty());
        // No other fixed id may carry more than its length, and the server
        // never sends such a thing.
        assert!(parse_match(Direction::ToServer, &[9, 3, 0x00, 0xDA, 1]).is_err());
        assert!(parse_match(Direction::ToClient, &[9, 3, 0x00, 0xE0, 1]).is_err());
    }

    #[test]
    fn the_lobby_sign_in_is_the_first_message() {
        // `2E <x> <u32 session> <bool>` in a single datagram.
        let datagram = [0x00, 0xFF, 0x00, 0x2E, 0x00, 0x44, 0x33, 0x22, 0x11, 0x01];
        let parsed = parse(Table::Lobby, Direction::ToServer, &datagram, 0).unwrap();
        let sign_in = parsed.messages[0];
        assert_eq!(sign_in.id, 0x2E);
        assert_eq!(sign_in.class, Class::Unique);
        assert_eq!(sign_in.x, Some(0));
        assert_eq!(sign_in.payload, [0x44, 0x33, 0x22, 0x11, 0x01]);
    }

    #[test]
    fn fixed_ids_in_a_multi_datagram_have_no_length_bytes() {
        // DC (9 B) + DF (5 B) + E1 (0 B) + FC, multi (ack0 bit 0).
        let mut datagram = vec![3, 2, 0x01, 0xDC];
        datagram.extend(1..=9);
        datagram.push(0xDF);
        datagram.extend([5, 4, 3, 2, 1]);
        datagram.extend([0xE1, 0xFC]);
        let parsed = parse_match(Direction::ToServer, &datagram).unwrap();
        let ids: Vec<u8> = parsed.messages.iter().map(|message| message.id).collect();
        assert_eq!(ids, [0xDC, 0xDF, 0xE1, 0xFC]);
        assert_eq!(parsed.messages[0].payload.len(), 9);
        assert_eq!(parsed.messages[1].payload, [5, 4, 3, 2, 1]);
        assert!(parsed.messages[2].payload.is_empty());
        assert!(parsed.messages[3].low_level);
    }

    #[test]
    fn variable_ids_in_a_multi_datagram_have_a_payload_length() {
        // Lobby: 2F x=1 (3 B) and 30 x=2 (300 B, `FF u16`), one tail byte.
        let mut datagram = vec![
            7, 6, 0x80, 0x2F, 1, 3, 0xA, 0xB, 0xC, 0x30, 2, 0xFF, 0x2C, 0x01,
        ];
        datagram.extend([0x55; 300]);
        datagram.push(0x01);
        let parsed = parse(Table::Lobby, Direction::ToServer, &datagram, 0).unwrap();
        assert_eq!(parsed.acks.tail, 1);
        assert_eq!(parsed.messages.len(), 2);
        assert_eq!(parsed.messages[0].payload, [0xA, 0xB, 0xC]);
        assert_eq!(parsed.messages[1].x, Some(2));
        assert_eq!(parsed.messages[1].payload, [0x55; 300]);
    }

    #[test]
    fn what_the_client_drops_is_an_error() {
        use ParseError::*;
        let to_client = |datagram: &[u8]| parse_match(Direction::ToClient, datagram).unwrap_err();
        assert_eq!(to_client(&[0, 0]), Header);
        assert_eq!(to_client(&[0, 0, 0]), EmptyMessageArea);
        assert_eq!(to_client(&[0, 0, 0x80, 0x00]), EmptyMessageArea);
        assert_eq!(to_client(&[0, 0, 0, 0xE2]), InvalidId(0xE2));
        assert_eq!(to_client(&[0, 0, 0, 0xD8, 0, 0, 0, 0, 0]), InvalidId(0xD8));
        assert_eq!(
            to_client(&[0, 0, 0, 0xE3, 1, 2, 3]),
            FixedMismatch {
                id: 0xE3,
                expected: 4,
                got: 3
            }
        );
        assert_eq!(
            to_client(&[0, 0, 0, 0xE3, 1, 2, 3, 4, 5]),
            FixedMismatch {
                id: 0xE3,
                expected: 4,
                got: 5
            }
        );
        assert_eq!(to_client(&[0, 0, 0, 0xFC, 0]), LowLevelNotLast);
        assert_eq!(to_client(&[0, 0, 1, 0xFC, 0xE9, 1, 2]), LowLevelNotLast);
        assert_eq!(to_client(&[0, 0, 1, 0xE9, 1]), Truncated);
        // E4 needs a byte, E8 more than two.
        assert_eq!(to_client(&[0, 0, 0, 0xE4, 0]), BadLength(0xE4));
        assert_eq!(to_client(&[0, 0, 0, 0xE8, 0, 1, 2]), BadLength(0xE8));
        assert!(parse_match(Direction::ToClient, &[0, 0, 0, 0xE8, 0, 1, 2, 3]).is_ok());
        // `FF u16` for a length that fits a byte.
        let mut short = vec![0, 0, 1, 0xF6, 0, 0xFF, 0x05, 0x00];
        short.extend([0; 5]);
        assert_eq!(to_client(&short), LongPrefixForShort);
        // The lobby's null entries.
        let lobby =
            |datagram: &[u8]| parse(Table::Lobby, Direction::ToClient, datagram, 0).unwrap_err();
        assert_eq!(lobby(&[0, 0, 0, 0xA7, 0]), NullId(0xA7));
        assert_eq!(lobby(&[0, 0, 0, 0x86, 0]), NullId(0x86));
        assert_eq!(lobby(&[0, 0, 0, 0x7F, 0]), InvalidId(0x7F));
        assert_eq!(lobby(&[0, 0, 0, 0xFF, 0x9F, 0, 1, 2]), PlainMultipart(0x9F));
        // 253 messages.
        let mut many = vec![0, 0, 1];
        many.extend([0xE1; 253]);
        assert_eq!(
            parse_match(Direction::ToServer, &many).unwrap_err(),
            TooManyMessages
        );
        many.pop();
        assert_eq!(
            parse_match(Direction::ToServer, &many)
                .unwrap()
                .messages
                .len(),
            252
        );
    }

    #[test]
    fn parts_carry_their_index_and_part_0_the_count() {
        let first = [0, 0, 0, 0xFF, 0x89, 4, 0, 3, 0xAA, 0xBB];
        let parsed = parse(Table::Lobby, Direction::ToClient, &first, 0).unwrap();
        let part = parsed.messages[0];
        assert_eq!(
            (part.id, part.class, part.x),
            (0x89, Class::Ordered, Some(4))
        );
        assert_eq!(
            part.part,
            Some(Part {
                index: 0,
                count: Some(3)
            })
        );
        assert_eq!(part.payload, [0xAA, 0xBB]);
        // A later part in a multi datagram has a length prefix.
        let later = [0, 0, 1, 0xFF, 0x89, 4, 2, 2, 0xCC, 0xDD, 0x9F, 1, 0x05];
        let parsed = parse(Table::Lobby, Direction::ToClient, &later, 0).unwrap();
        assert_eq!(
            parsed.messages[0].part,
            Some(Part {
                index: 2,
                count: None
            })
        );
        assert_eq!(parsed.messages[0].payload, [0xCC, 0xDD]);
        assert_eq!(parsed.messages[1].id, 0x9F);
        assert_eq!(parsed.messages[1].x, None);
        assert_eq!(parsed.messages[1].payload, [0x05]);
    }
}
