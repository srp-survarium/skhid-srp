//! Datagram body: messages in single or multi format (PROVEN at 0x200755D40).
//!
//! One message: type byte `t`. `FF` = multipart prefix, the real id follows. `FA..=FD` = low-level, one
//! byte. Otherwise `id [x] [part [count]] data`:
//! * x (u8) when the id is unique or ordered (not plain);
//! * with the FF prefix: u8 part index, and u8 part count when the index is 0;
//! * single datagram: data is the rest of the datagram; multi datagram: `u8 len` (<= 0xFE) or
//!   `FF u16 len` first (payload bytes only), then the data.
//!
//! Any failure drops the whole datagram in the client; [`parse`] returns an error for the same cases.

use crate::ack::{self, Header, HeaderError};
use crate::table::{class_of, low_level, Class, Direction, MULTIPART};
use std::fmt;

/// A datagram is at most 0x1FC = 508 bytes (tail acks included).
pub const MAX_DATAGRAM: usize = 0x1FC;
/// Payload bytes per part (~440 B parts work with the real client).
pub const PART: usize = 440;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Part {
    pub index: u8,
    /// Only part 0 carries the count.
    pub count: Option<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Data {
    pub id: u8,
    /// Unique / order id; `None` for plain ids.
    pub x: Option<u8>,
    pub part: Option<Part>,
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    LowLevel(u8),
    Data(Data),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Datagram {
    pub header: Header,
    pub messages: Vec<Message>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseError {
    Header(HeaderError),
    /// "invalid message type": outside the table's range, or 0xFE.
    InvalidType(u8),
    /// Ran out of bytes inside a message.
    Truncated,
    /// Encode: a message that cannot be written in this format.
    Unencodable(&'static str),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::Header(e) => write!(f, "header: {e}"),
            ParseError::InvalidType(t) => write!(f, "invalid message type {t:#04x}"),
            ParseError::Truncated => write!(f, "truncated message"),
            ParseError::Unencodable(why) => write!(f, "cannot encode: {why}"),
        }
    }
}

impl std::error::Error for ParseError {}

impl From<HeaderError> for ParseError {
    fn from(e: HeaderError) -> Self {
        ParseError::Header(e)
    }
}

/// Parse one datagram travelling in `direction` (which table applies).
pub fn parse(direction: Direction, d: &[u8]) -> Result<Datagram, ParseError> {
    let (header, end) = ack::decode(d)?;
    let area = &d[3..end];
    let multi = header.multi;
    let mut messages = Vec::new();
    let mut i = 0;
    while i < area.len() {
        let t = area[i];
        i += 1;
        if low_level::is_low_level(t) {
            messages.push(Message::LowLevel(t));
            continue;
        }
        let prefixed = t == MULTIPART;
        let id = if prefixed {
            let id = *area.get(i).ok_or(ParseError::Truncated)?;
            i += 1;
            id
        } else {
            t
        };
        let class = class_of(direction, id).ok_or(ParseError::InvalidType(id))?;
        let x = if class.has_x() {
            let x = *area.get(i).ok_or(ParseError::Truncated)?;
            i += 1;
            Some(x)
        } else {
            None
        };
        let part = if prefixed {
            let index = *area.get(i).ok_or(ParseError::Truncated)?;
            i += 1;
            let count = if index == 0 {
                let c = *area.get(i).ok_or(ParseError::Truncated)?;
                i += 1;
                Some(c)
            } else {
                None
            };
            Some(Part { index, count })
        } else {
            None
        };
        let payload = if multi {
            let mut len = usize::from(*area.get(i).ok_or(ParseError::Truncated)?);
            i += 1;
            if len == 0xFF {
                let b = area.get(i..i + 2).ok_or(ParseError::Truncated)?;
                len = usize::from(u16::from_le_bytes([b[0], b[1]]));
                i += 2;
            }
            let p = area.get(i..i + len).ok_or(ParseError::Truncated)?;
            i += len;
            p
        } else {
            let p = &area[i..];
            i = area.len();
            p
        };
        messages.push(Message::Data(Data { id, x, part, payload: payload.to_vec() }));
    }
    Ok(Datagram { header, messages })
}

/// The message area of `messages` in the format `multi` says.
pub fn encode_area(
    direction: Direction,
    multi: bool,
    messages: &[Message],
) -> Result<Vec<u8>, ParseError> {
    let mut out = Vec::new();
    for (k, m) in messages.iter().enumerate() {
        match m {
            Message::LowLevel(t) => {
                if !low_level::is_low_level(*t) {
                    return Err(ParseError::InvalidType(*t));
                }
                out.push(*t);
            }
            Message::Data(d) => {
                let class = class_of(direction, d.id).ok_or(ParseError::InvalidType(d.id))?;
                if d.x.is_some() != class.has_x() {
                    return Err(ParseError::Unencodable("x byte does not match the id's class"));
                }
                if d.part.is_some() {
                    out.push(MULTIPART);
                }
                out.push(d.id);
                out.extend(d.x);
                if let Some(p) = d.part {
                    out.push(p.index);
                    if p.index == 0 {
                        out.push(p.count.ok_or(ParseError::Unencodable("part 0 needs a count"))?);
                    }
                }
                if multi {
                    if d.payload.len() < 0xFF {
                        out.push(d.payload.len() as u8);
                    } else {
                        let len = u16::try_from(d.payload.len())
                            .map_err(|_| ParseError::Unencodable("payload over 65535 bytes"))?;
                        out.push(0xFF);
                        out.extend(len.to_le_bytes());
                    }
                } else if k + 1 != messages.len() {
                    return Err(ParseError::Unencodable("data message not last in a single datagram"));
                }
                out.extend(&d.payload);
            }
        }
    }
    Ok(out)
}

/// Encode a datagram (the header's `multi` flag picks the format).
pub fn encode(direction: Direction, dg: &Datagram) -> Result<Vec<u8>, ParseError> {
    let area = encode_area(direction, dg.header.multi, &dg.messages)?;
    Ok(ack::encode(&dg.header, &area)?)
}

/// Split one message into parts of at most `part` payload bytes (normally [`PART`]). Each part is a
/// [`Data`] ready for a single-format datagram; all parts share x. `None` if more than 255 parts.
pub fn split(
    direction: Direction,
    id: u8,
    x: Option<u8>,
    payload: &[u8],
    part: usize,
) -> Option<Vec<Data>> {
    let chunks: Vec<&[u8]> = if payload.is_empty() { vec![&[][..]] } else { payload.chunks(part).collect() };
    if chunks.len() > 255 || class_of(direction, id).is_none() {
        return None;
    }
    let count = chunks.len() as u8;
    Some(
        chunks
            .into_iter()
            .enumerate()
            .map(|(i, c)| Data {
                id,
                x,
                part: Some(Part { index: i as u8, count: (i == 0).then_some(count) }),
                payload: c.to_vec(),
            })
            .collect(),
    )
}

/// Class helper for callers that hold a [`Data`].
pub fn class(direction: Direction, d: &Data) -> Option<Class> {
    class_of(direction, d.id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect()
    }

    #[test]
    fn single_sign_in() {
        let dg = parse(Direction::ToServer, &hex("00ff042e0078563412ffff")).unwrap();
        assert_eq!(dg.messages.len(), 1);
        match &dg.messages[0] {
            Message::Data(d) => {
                assert_eq!((d.id, d.x), (0x2E, Some(0)));
                assert_eq!(d.payload, hex("78563412ffff"));
            }
            m => panic!("{m:?}"),
        }
        assert_eq!(encode(Direction::ToServer, &dg).unwrap(), hex("00ff042e0078563412ffff"));
    }

    #[test]
    fn multi_format_round_trip() {
        // two messages: 4F x=2 (3 bytes), 32 x=3 (2 bytes), plus a keepalive
        let area = hex("4f0203aabbcc3203020001fc");
        let h = Header::new(1, 0x1FF, true, &Default::default(), 0);
        let d = ack::encode(&h, &area).unwrap();
        let dg = parse(Direction::ToServer, &d).unwrap();
        assert_eq!(dg.messages.len(), 3);
        assert_eq!(dg.messages[2], Message::LowLevel(0xFC));
        assert_eq!(encode(Direction::ToServer, &dg).unwrap(), d);
    }

    #[test]
    fn multipart_parts_and_plain_without_x() {
        let h = Header::new(5, 1, false, &Default::default(), 4);
        let part0 = Data { id: 0x87, x: Some(4), part: Some(Part { index: 0, count: Some(3) }), payload: vec![1, 2] };
        let d = encode(Direction::ToClient, &Datagram { header: h.clone(), messages: vec![Message::Data(part0.clone())] }).unwrap();
        assert_eq!(&d[3..8], &[0xFF, 0x87, 4, 0, 3]);
        assert_eq!(parse(Direction::ToClient, &d).unwrap().messages, vec![Message::Data(part0)]);
        // AC is plain: no x byte
        let ac = Data { id: 0xAC, x: None, part: None, payload: vec![0x3c, 0, 0, 0, 0, 0, 0, 0] };
        let d = encode(Direction::ToClient, &Datagram { header: h, messages: vec![Message::Data(ac.clone())] }).unwrap();
        assert_eq!(d[3], 0xAC);
        assert_eq!(d[4], 0x3c);
        assert_eq!(parse(Direction::ToClient, &d).unwrap().messages, vec![Message::Data(ac)]);
    }

    #[test]
    fn long_length_prefix() {
        let big = Data { id: 0x50, x: Some(1), part: None, payload: vec![7; 541] };
        let h = Header::new(0, 0x1FF, true, &Default::default(), 0);
        let dg = Datagram { header: h, messages: vec![Message::Data(big.clone())] };
        let d = encode(Direction::ToServer, &dg).unwrap();
        assert_eq!(&d[3..8], &[0x50, 1, 0xFF, 0x1D, 0x02]);
        assert_eq!(parse(Direction::ToServer, &d).unwrap(), dg);
    }

    #[test]
    fn errors() {
        // id 0x2D is outside the C->S range
        let h = Header::new(0, 0x1FF, false, &Default::default(), 0);
        let d = ack::encode(&h, &[0x2D, 0, 1]).unwrap();
        assert_eq!(parse(Direction::ToServer, &d), Err(ParseError::InvalidType(0x2D)));
        // FE is invalid
        let d = ack::encode(&h, &[0xFE]).unwrap();
        assert_eq!(parse(Direction::ToServer, &d), Err(ParseError::InvalidType(0xFE)));
        // multi message with a length past the end
        let hm = Header::new(0, 0x1FF, true, &Default::default(), 0);
        let d = ack::encode(&hm, &[0x4F, 0, 9, 1]).unwrap();
        assert_eq!(parse(Direction::ToServer, &d), Err(ParseError::Truncated));
    }

    #[test]
    fn split_is_even() {
        let p: Vec<u8> = (0..960u32).map(|i| i as u8).collect();
        let parts = split(Direction::ToClient, 0x87, Some(4), &p, PART).unwrap();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0].part, Some(Part { index: 0, count: Some(3) }));
        assert_eq!(parts[1].part, Some(Part { index: 1, count: None }));
        assert_eq!(parts[0].payload.len(), 440);
        assert_eq!(parts[2].payload.len(), 80);
    }
}
