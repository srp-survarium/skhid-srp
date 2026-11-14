//! The peer wire protocol (BEP 3), as far as this updater speaks it: the
//! handshake and the nine messages of BEP 3, no extensions (it sets no
//! reserved bit, so a peer sends none of theirs; one that does is refused).
//! Peers are strangers: every message is read once, against fixed limits
//! and the torrent's own numbers, into [`Message`], and anything else ends
//! the connection.

use alloc::vec::Vec;

use crate::torrent::BLOCK;

pub const PROTOCOL: &[u8; 19] = b"BitTorrent protocol";
pub const HANDSHAKE_LEN: usize = 68;

/// The handshake this end sends.
pub fn handshake(info_hash: &[u8; 20], peer_id: &[u8; 20]) -> [u8; HANDSHAKE_LEN] {
    let mut out = [0_u8; HANDSHAKE_LEN];
    out[0] = 19;
    out[1..20].copy_from_slice(PROTOCOL);
    out[28..48].copy_from_slice(info_hash);
    out[48..68].copy_from_slice(peer_id);
    out
}

/// A peer's handshake: its peer id, if it is this torrent's.
pub fn their_handshake(bytes: &[u8; HANDSHAKE_LEN], info_hash: &[u8; 20]) -> Option<[u8; 20]> {
    (bytes[0] == 19 && &bytes[1..20] == PROTOCOL && &bytes[28..48] == info_hash)
        .then(|| bytes[48..68].try_into().expect("20 bytes"))
}

/// A request: a block of a piece.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Block {
    pub piece: u32,
    pub begin: u32,
    pub length: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    KeepAlive,
    Choke,
    Unchoke,
    Interested,
    NotInterested,
    Have(u32),
    /// One bit a piece, the first piece's the high bit of the first byte.
    Bitfield(Vec<u8>),
    Request(Block),
    Piece { block: Block, data: Vec<u8> },
    Cancel(Block),
}

/// What a peer sent that is refused: the connection ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Refused;

/// The numbers a message is checked against: the torrent's.
#[derive(Debug, Clone, Copy)]
pub struct Shape {
    pub pieces: u32,
    pub piece_length: u32,
    /// The last piece's size (it may be shorter).
    pub last_piece: u32,
}

impl Shape {
    pub fn piece_size(&self, piece: u32) -> u32 {
        if piece + 1 == self.pieces {
            self.last_piece
        } else {
            self.piece_length
        }
    }

    pub fn bitfield_len(&self) -> usize {
        self.pieces.div_ceil(8) as usize
    }

    /// The longest message a peer may send: a block, or the bitfield.
    pub fn max_message(&self) -> usize {
        (9 + BLOCK as usize).max(1 + self.bitfield_len())
    }

    fn block(&self, bytes: &[u8], length: Option<u32>) -> Result<Block, Refused> {
        let word = |at: usize| u32::from_be_bytes(bytes[at..at + 4].try_into().expect("4 bytes"));
        let block = Block {
            piece: word(0),
            begin: word(4),
            length: length.unwrap_or_else(|| word(8)),
        };
        let fits = block.piece < self.pieces
            && block.length > 0
            && block.length <= BLOCK
            && block.begin % BLOCK == 0
            && u64::from(block.begin) + u64::from(block.length)
                <= u64::from(self.piece_size(block.piece));
        fits.then_some(block).ok_or(Refused)
    }
}

/// One message's bytes (its id and fields, without its length).
pub fn parse(body: &[u8], shape: &Shape) -> Result<Message, Refused> {
    let Some((&id, fields)) = body.split_first() else {
        return Ok(Message::KeepAlive);
    };
    let exactly = |n: usize| if fields.len() == n { Ok(()) } else { Err(Refused) };
    Ok(match id {
        0 => exactly(0).map(|()| Message::Choke)?,
        1 => exactly(0).map(|()| Message::Unchoke)?,
        2 => exactly(0).map(|()| Message::Interested)?,
        3 => exactly(0).map(|()| Message::NotInterested)?,
        4 => {
            exactly(4)?;
            let piece = u32::from_be_bytes(fields.try_into().expect("4 bytes"));
            if piece >= shape.pieces {
                return Err(Refused);
            }
            Message::Have(piece)
        }
        5 => {
            exactly(shape.bitfield_len())?;
            // The bits past the last piece are zero (BEP 3).
            let spare = shape.bitfield_len() as u32 * 8 - shape.pieces;
            if fields.last().is_some_and(|last| last & ((1_u16 << spare) - 1) as u8 != 0) {
                return Err(Refused);
            }
            Message::Bitfield(fields.into())
        }
        6 => {
            exactly(12)?;
            Message::Request(shape.block(fields, None)?)
        }
        7 => {
            if fields.len() < 9 {
                return Err(Refused);
            }
            let block = shape.block(fields, Some((fields.len() - 8) as u32))?;
            Message::Piece {
                block,
                data: fields[8..].into(),
            }
        }
        8 => {
            exactly(12)?;
            Message::Cancel(shape.block(fields, None)?)
        }
        _ => return Err(Refused),
    })
}

/// A message as it goes on the wire: its length, id and fields.
pub fn frame(message: &Message) -> Vec<u8> {
    let mut out = alloc::vec![0_u8; 4];
    let block = |out: &mut Vec<u8>, block: &Block, with_length: bool| {
        out.extend_from_slice(&block.piece.to_be_bytes());
        out.extend_from_slice(&block.begin.to_be_bytes());
        if with_length {
            out.extend_from_slice(&block.length.to_be_bytes());
        }
    };
    match message {
        Message::KeepAlive => {}
        Message::Choke => out.push(0),
        Message::Unchoke => out.push(1),
        Message::Interested => out.push(2),
        Message::NotInterested => out.push(3),
        Message::Have(piece) => {
            out.push(4);
            out.extend_from_slice(&piece.to_be_bytes());
        }
        Message::Bitfield(bits) => {
            out.push(5);
            out.extend_from_slice(bits);
        }
        Message::Request(request) => {
            out.push(6);
            block(&mut out, request, true);
        }
        Message::Piece { block: piece, data } => {
            out.push(7);
            block(&mut out, piece, false);
            out.extend_from_slice(data);
        }
        Message::Cancel(request) => {
            out.push(8);
            block(&mut out, request, true);
        }
    }
    let length = (out.len() - 4) as u32;
    out[..4].copy_from_slice(&length.to_be_bytes());
    out
}

/// Cuts the bytes of a connection into messages, as they come.
#[derive(Debug, Default)]
pub struct Reader {
    buffer: Vec<u8>,
}

impl Reader {
    pub fn push(&mut self, bytes: &[u8]) {
        self.buffer.extend_from_slice(bytes);
    }

    /// The handshake, once all of its bytes are here.
    pub fn handshake(&mut self) -> Option<[u8; HANDSHAKE_LEN]> {
        if self.buffer.len() < HANDSHAKE_LEN {
            return None;
        }
        let bytes = self.buffer[..HANDSHAKE_LEN].try_into().expect("68 bytes");
        self.buffer.drain(..HANDSHAKE_LEN);
        Some(bytes)
    }

    /// The next whole message. A length past what the torrent allows is
    /// refused before its bytes are waited for.
    pub fn message(&mut self, shape: &Shape) -> Result<Option<Message>, Refused> {
        let Some(head) = self.buffer.get(..4) else {
            return Ok(None);
        };
        let length = u32::from_be_bytes(head.try_into().expect("4 bytes")) as usize;
        if length > shape.max_message() {
            return Err(Refused);
        }
        if self.buffer.len() < 4 + length {
            return Ok(None);
        }
        let message = parse(&self.buffer[4..4 + length], shape);
        self.buffer.drain(..4 + length);
        message.map(Some)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHAPE: Shape = Shape {
        pieces: 10,
        piece_length: 32 * 1024,
        last_piece: 100,
    };

    #[test]
    fn messages_go_and_come_back() {
        let block = Block {
            piece: 9,
            begin: 0,
            length: 100,
        };
        for message in [
            Message::KeepAlive,
            Message::Choke,
            Message::Unchoke,
            Message::Interested,
            Message::NotInterested,
            Message::Have(3),
            Message::Bitfield(alloc::vec![0xFF, 0xC0]),
            Message::Request(block),
            Message::Cancel(block),
            Message::Piece {
                block,
                data: alloc::vec![7; 100],
            },
        ] {
            let mut reader = Reader::default();
            let bytes = frame(&message);
            for byte in &bytes[..bytes.len() - 1] {
                reader.push(core::slice::from_ref(byte));
                assert_eq!(reader.message(&SHAPE), Ok(None));
            }
            reader.push(&bytes[bytes.len() - 1..]);
            assert_eq!(reader.message(&SHAPE), Ok(Some(message)));
        }
    }

    #[test]
    fn what_a_stranger_may_not_send() {
        let request = |piece: u32, begin: u32, length: u32| {
            let mut body = alloc::vec![6];
            for n in [piece, begin, length] {
                body.extend_from_slice(&n.to_be_bytes());
            }
            body
        };
        for body in [
            alloc::vec![0, 0],                 // a choke with a field
            alloc::vec![4, 0, 0, 0, 10],       // have past the last piece
            alloc::vec![5, 0xFF],              // a bitfield of another size
            alloc::vec![5, 0xFF, 0xE0],        // a spare bit set
            request(9, 0, 101),                // past the last piece's end
            request(0, 0, BLOCK + 1),          // a block too long
            request(0, 1, 10),                 // not at a block
            request(0, 0, 0),                  // nothing
            alloc::vec![20, 0, 1],             // an extension message
            alloc::vec![9, 0x1A, 0xE1],        // DHT port: no bit set for it
        ] {
            assert_eq!(parse(&body, &SHAPE), Err(Refused), "{body:?}");
        }
        let mut reader = Reader::default();
        reader.push(&(BLOCK + 10).to_be_bytes());
        assert_eq!(reader.message(&SHAPE), Err(Refused));
    }

    #[test]
    fn the_handshake() {
        let ours = handshake(&[1; 20], &[2; 20]);
        assert_eq!(their_handshake(&ours, &[1; 20]), Some([2; 20]));
        assert_eq!(their_handshake(&ours, &[3; 20]), None);
        let mut other = ours;
        other[5] = b'x';
        assert_eq!(their_handshake(&other, &[1; 20]), None);
    }
}
