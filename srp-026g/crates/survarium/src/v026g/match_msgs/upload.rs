//! The patched client's snapshot upload
//! (`docs/notes/client_patch.md` §3.2, §3.4).
//!
//! A stock client never sends its world. The srp patch does, as chunks in
//! client→server 0xE0 (the leave message, which is empty when real):
//!
//! ```text
//! u32 item_time, u16 offset, u16 total, data[min(480, total - offset)]
//! ```
//!
//! once unasked when the client is ready (the initial world), and again on
//! each server→client 0xF6 (the client's event-horizon world). A chunk is
//! always alone in its datagram, resent every 500 ms until acked, with no
//! transport dedup and in any order. When all `total` bytes are there the
//! body is `game_state_history_item::serialize`'s output: what
//! [`super::snapshot`] reads, what the client's 0xDF hashes, and what 0xE8
//! sends back out.

use std::collections::{BTreeMap, VecDeque};

/// The data bytes of every chunk but the last.
pub const CHUNK_DATA: usize = 480;
/// `u32 item_time, u16 offset, u16 total`.
pub const CHUNK_HEADER: usize = 8;
/// The client serializes its world into a buffer of this size: 0x8000 in
/// the stock exe, 0xFF00 with patch 60 (`client_patch.md` §3.54), the
/// most whole chunks a `u16 total` holds (136 of 480).
pub const MAX_TOTAL: usize = 0xFF00;
/// The chunks of the largest world.
pub const MAX_CHUNKS: usize = MAX_TOTAL.div_ceil(CHUNK_DATA);

/// Why a 0xE0 payload is no chunk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChunkError {
    /// Shorter than the header and one byte.
    TooShort,
    /// `total` is 0 or over [`MAX_TOTAL`].
    Total(u16),
    /// `offset` is no multiple of 480, or not below `total`.
    Offset { offset: u16, total: u16 },
    /// Not `8 + min(480, total − offset)` bytes.
    Length { expected: usize, got: usize },
}

impl std::fmt::Display for ChunkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooShort => write!(f, "too short"),
            Self::Total(total) => write!(f, "a total of {total} bytes"),
            Self::Offset { offset, total } => write!(f, "offset {offset} of {total}"),
            Self::Length { expected, got } => write!(f, "{got} bytes, not {expected}"),
        }
    }
}

impl std::error::Error for ChunkError {}

/// One chunk of an upload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chunk<'a> {
    /// The time of the world it is a part of.
    pub item_time: u32,
    pub offset: u16,
    /// The whole body's length.
    pub total: u16,
    pub data: &'a [u8],
}

impl<'a> Chunk<'a> {
    /// Reads a non-empty 0xE0 payload: `offset % 480 == 0`,
    /// `offset < total ≤ MAX_TOTAL` and exactly the length those two give.
    pub fn parse(payload: &'a [u8]) -> Result<Self, ChunkError> {
        if payload.len() <= CHUNK_HEADER {
            return Err(ChunkError::TooShort);
        }
        let item_time = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
        let offset = u16::from_le_bytes([payload[4], payload[5]]);
        let total = u16::from_le_bytes([payload[6], payload[7]]);
        if total == 0 || usize::from(total) > MAX_TOTAL {
            return Err(ChunkError::Total(total));
        }
        if !usize::from(offset).is_multiple_of(CHUNK_DATA) || offset >= total {
            return Err(ChunkError::Offset { offset, total });
        }
        let expected = CHUNK_HEADER + CHUNK_DATA.min(usize::from(total - offset));
        if payload.len() != expected {
            return Err(ChunkError::Length {
                expected,
                got: payload.len(),
            });
        }
        Ok(Self {
            item_time,
            offset,
            total,
            data: &payload[CHUNK_HEADER..],
        })
    }

    /// The 0xE0 payload of this chunk.
    pub fn payload(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(CHUNK_HEADER + self.data.len());
        out.extend(self.item_time.to_le_bytes());
        out.extend(self.offset.to_le_bytes());
        out.extend(self.total.to_le_bytes());
        out.extend_from_slice(self.data);
        out
    }
}

/// The 0xE0 payloads a patched client sends for a world: what the injected
/// code does, for tests and the client model. `None`: the body is empty or
/// over [`MAX_TOTAL`] bytes (the client asserts on such a world).
pub fn split(item_time: u32, body: &[u8]) -> Option<Vec<Vec<u8>>> {
    if body.is_empty() || body.len() > MAX_TOTAL {
        return None;
    }
    let total = body.len() as u16;
    Some(
        body.chunks(CHUNK_DATA)
            .enumerate()
            .map(|(index, data)| {
                Chunk {
                    item_time,
                    offset: (index * CHUNK_DATA) as u16,
                    total,
                    data,
                }
                .payload()
            })
            .collect(),
    )
}

/// A complete upload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Upload {
    pub item_time: u32,
    /// `u32 active_clients_mask`, the player blocks, the six sections.
    pub body: Vec<u8>,
}

/// What a chunk did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pushed {
    /// New; the upload still misses chunks.
    Stored,
    /// A resend of a chunk already here, or of an upload already complete.
    Duplicate,
    /// The last missing chunk: the whole body.
    Complete(Upload),
}

struct Partial {
    data: Vec<u8>,
    /// The offsets that came.
    chunks: BTreeMap<u16, ()>,
}

/// One client's uploads, put together. Keyed by `item_time`: a new request
/// can give a new time while chunks of the last one are still resent.
pub struct Reassembler {
    partial: BTreeMap<u32, Partial>,
    /// The order the partial uploads began in, oldest first.
    began: VecDeque<u32>,
    /// The item times lately completed, so that their resends start nothing.
    completed: VecDeque<u32>,
}

/// Unfinished uploads kept for one client; the oldest goes first.
const MAX_PARTIAL: usize = 4;
/// Completed item times remembered.
const COMPLETED_KEPT: usize = 16;

impl Default for Reassembler {
    fn default() -> Self {
        Self::new()
    }
}

impl Reassembler {
    pub fn new() -> Self {
        Self {
            partial: BTreeMap::new(),
            began: VecDeque::new(),
            completed: VecDeque::new(),
        }
    }

    /// Whether an upload is under way.
    pub fn is_receiving(&self) -> bool {
        !self.partial.is_empty()
    }

    /// The bytes still missing of the upload of `item_time`, if one is
    /// under way.
    pub fn missing(&self, item_time: u32) -> Option<usize> {
        let partial = self.partial.get(&item_time)?;
        let received: usize = partial
            .chunks
            .keys()
            .map(|&offset| CHUNK_DATA.min(partial.data.len() - usize::from(offset)))
            .sum();
        Some(partial.data.len() - received)
    }

    /// Drops every unfinished upload (the client left, or the server gave
    /// up on it).
    pub fn clear(&mut self) {
        self.partial.clear();
        self.began.clear();
    }

    /// Takes a chunk, placed by its offset; duplicates by
    /// `(item_time, offset)` are dropped.
    pub fn push(&mut self, chunk: &Chunk<'_>) -> Pushed {
        if self.completed.contains(&chunk.item_time) {
            return Pushed::Duplicate;
        }
        let total = usize::from(chunk.total);
        // The same world serializes to the same length; another total for a
        // known time is an upload begun anew [GUESS: not seen in the patch].
        if self
            .partial
            .get(&chunk.item_time)
            .is_some_and(|partial| partial.data.len() != total)
        {
            self.forget(chunk.item_time);
        }
        if !self.partial.contains_key(&chunk.item_time) {
            while self.began.len() >= MAX_PARTIAL {
                if let Some(oldest) = self.began.pop_front() {
                    self.partial.remove(&oldest);
                }
            }
            self.began.push_back(chunk.item_time);
        }
        let partial = self
            .partial
            .entry(chunk.item_time)
            .or_insert_with(|| Partial {
                data: vec![0; total],
                chunks: BTreeMap::new(),
            });
        if partial.chunks.contains_key(&chunk.offset) {
            return Pushed::Duplicate;
        }
        let start = usize::from(chunk.offset);
        // `Chunk::parse` made the chunk fit; one built by hand may not.
        let Some(target) = partial.data.get_mut(start..start + chunk.data.len()) else {
            return Pushed::Duplicate;
        };
        target.copy_from_slice(chunk.data);
        partial.chunks.insert(chunk.offset, ());
        if partial.chunks.len() < total.div_ceil(CHUNK_DATA) {
            return Pushed::Stored;
        }
        let body = self
            .partial
            .remove(&chunk.item_time)
            .map(|partial| partial.data)
            .unwrap_or_default();
        self.began.retain(|&time| time != chunk.item_time);
        if self.completed.len() >= COMPLETED_KEPT {
            self.completed.pop_front();
        }
        self.completed.push_back(chunk.item_time);
        Pushed::Complete(Upload {
            item_time: chunk.item_time,
            body,
        })
    }

    fn forget(&mut self, item_time: u32) {
        self.partial.remove(&item_time);
        self.began.retain(|&time| time != item_time);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world(len: usize) -> Vec<u8> {
        (0..len).map(|i| (i * 7 + i / 480) as u8).collect()
    }

    #[test]
    fn a_chunk_is_time_offset_total_and_data() {
        // client_patch.md §3.4: `u32 item_time, u16 offset, u16 total, data`.
        // The second chunk of a 500-byte world at 50 ms: 20 bytes of data.
        let mut payload = vec![0x32, 0, 0, 0, 0xE0, 0x01, 0xF4, 0x01];
        payload.extend([0xAB; 20]);
        let chunk = Chunk::parse(&payload).unwrap();
        assert_eq!(chunk.item_time, 50);
        assert_eq!(chunk.offset, 480);
        assert_eq!(chunk.total, 500);
        assert_eq!(chunk.data, [0xAB; 20]);
        assert_eq!(chunk.payload(), payload);
    }

    #[test]
    fn a_chunk_must_be_exactly_what_its_header_says() {
        let chunk = |offset: u16, total: u16, data: usize| {
            let mut payload = vec![0x32, 0, 0, 0];
            payload.extend(offset.to_le_bytes());
            payload.extend(total.to_le_bytes());
            payload.extend(vec![0; data]);
            Chunk::parse(&payload).map(|_| ())
        };
        assert_eq!(chunk(0, 1, 1), Ok(()));
        assert_eq!(chunk(0, 480, 480), Ok(()));
        assert_eq!(chunk(0, 481, 480), Ok(()));
        assert_eq!(chunk(480, 481, 1), Ok(()));
        assert_eq!(chunk(0x7F80, 0x8000, 128), Ok(()), "the stock exe's 69th chunk");
        assert_eq!(chunk(0xFD20, 0xFF00, 480), Ok(()), "the 136th chunk");
        assert_eq!(chunk(0xFD20, 0xFFFF, 480), Err(ChunkError::Total(0xFFFF)));
        // The header alone, or less.
        assert_eq!(chunk(0, 1, 0), Err(ChunkError::TooShort));
        assert_eq!(Chunk::parse(&[1, 2, 3]), Err(ChunkError::TooShort));
        // Totals.
        assert_eq!(chunk(0, 0, 1), Err(ChunkError::Total(0)));
        assert_eq!(chunk(0, 0xFF01, 480), Err(ChunkError::Total(0xFF01)));
        // Offsets: off the 480 grid, at or past the total.
        assert_eq!(
            chunk(1, 500, 480),
            Err(ChunkError::Offset {
                offset: 1,
                total: 500
            })
        );
        assert_eq!(
            chunk(480, 480, 1),
            Err(ChunkError::Offset {
                offset: 480,
                total: 480
            })
        );
        assert_eq!(
            chunk(960, 500, 1),
            Err(ChunkError::Offset {
                offset: 960,
                total: 500
            })
        );
        // Lengths: a short middle chunk, a long last one.
        assert_eq!(
            chunk(0, 1000, 479),
            Err(ChunkError::Length {
                expected: 488,
                got: 487
            })
        );
        assert_eq!(
            chunk(480, 500, 21),
            Err(ChunkError::Length {
                expected: 28,
                got: 29
            })
        );
    }

    #[test]
    fn the_largest_world_is_136_chunks() {
        // Patch 60: 0xFF00 bytes, 136 whole chunks; the last at 0xFD20.
        assert_eq!(MAX_CHUNKS, 136);
        let body = world(MAX_TOTAL);
        let chunks = split(100, &body).unwrap();
        assert_eq!(chunks.len(), 136);
        assert_eq!(chunks[0].len(), 488);
        assert_eq!(chunks[135].len(), 488);
        assert_eq!(&chunks[135][4..8], [0x20, 0xFD, 0x00, 0xFF]);
        let mut reassembler = Reassembler::new();
        let mut pushed = Vec::new();
        for chunk in chunks.iter().rev() {
            pushed.push(reassembler.push(&Chunk::parse(chunk).unwrap()));
        }
        assert!(pushed[..135].iter().all(|p| *p == Pushed::Stored));
        assert_eq!(
            pushed[135],
            Pushed::Complete(Upload {
                item_time: 100,
                body
            })
        );
        assert_eq!(split(100, &world(MAX_TOTAL + 1)), None);
        assert_eq!(split(100, &[]), None);
    }

    #[test]
    fn chunks_in_order_give_the_body() {
        let body = world(1000);
        let mut reassembler = Reassembler::new();
        let chunks = split(50, &body).unwrap();
        assert_eq!(chunks.len(), 3);
        assert_eq!(
            reassembler.push(&Chunk::parse(&chunks[0]).unwrap()),
            Pushed::Stored
        );
        assert!(reassembler.is_receiving());
        assert_eq!(reassembler.missing(50), Some(520));
        assert_eq!(
            reassembler.push(&Chunk::parse(&chunks[1]).unwrap()),
            Pushed::Stored
        );
        assert_eq!(reassembler.missing(50), Some(40));
        assert_eq!(
            reassembler.push(&Chunk::parse(&chunks[2]).unwrap()),
            Pushed::Complete(Upload {
                item_time: 50,
                body
            })
        );
        assert!(!reassembler.is_receiving());
        assert_eq!(reassembler.missing(50), None);
    }

    #[test]
    fn chunks_out_of_order_and_resent_give_the_body_once() {
        let body = world(2000);
        let chunks = split(1_250, &body).unwrap();
        assert_eq!(chunks.len(), 5);
        let mut reassembler = Reassembler::new();
        let mut complete = Vec::new();
        // Last first, each resent, then every one resent again.
        for index in [4, 4, 2, 0, 2, 3, 0, 1, 1, 0, 1, 2, 3, 4] {
            match reassembler.push(&Chunk::parse(&chunks[index]).unwrap()) {
                Pushed::Complete(upload) => complete.push(upload),
                Pushed::Stored | Pushed::Duplicate => {}
            }
        }
        assert_eq!(
            complete,
            [Upload {
                item_time: 1_250,
                body
            }]
        );
        assert!(!reassembler.is_receiving(), "the resends began nothing");
    }

    #[test]
    fn duplicates_are_told_apart_from_new_chunks() {
        let chunks = split(50, &world(1000)).unwrap();
        let mut reassembler = Reassembler::new();
        let first = Chunk::parse(&chunks[0]).unwrap();
        assert_eq!(reassembler.push(&first), Pushed::Stored);
        assert_eq!(reassembler.push(&first), Pushed::Duplicate);
    }

    #[test]
    fn uploads_of_two_times_do_not_mix() {
        let old = world(600);
        let new: Vec<u8> = world(700).iter().map(|byte| !byte).collect();
        let old_chunks = split(1_000, &old).unwrap();
        let new_chunks = split(2_000, &new).unwrap();
        let mut reassembler = Reassembler::new();
        let mut push = |payload: &Vec<u8>| reassembler.push(&Chunk::parse(payload).unwrap());
        assert_eq!(push(&old_chunks[0]), Pushed::Stored);
        assert_eq!(push(&new_chunks[1]), Pushed::Stored);
        assert_eq!(
            push(&old_chunks[1]),
            Pushed::Complete(Upload {
                item_time: 1_000,
                body: old
            })
        );
        assert_eq!(
            push(&new_chunks[0]),
            Pushed::Complete(Upload {
                item_time: 2_000,
                body: new
            })
        );
    }

    #[test]
    fn a_one_chunk_world_completes_at_once() {
        let body = world(480);
        let chunks = split(50, &body).unwrap();
        assert_eq!(chunks.len(), 1);
        let mut reassembler = Reassembler::new();
        assert_eq!(
            reassembler.push(&Chunk::parse(&chunks[0]).unwrap()),
            Pushed::Complete(Upload {
                item_time: 50,
                body
            })
        );
    }

    #[test]
    fn another_total_for_the_same_time_starts_over() {
        let mut reassembler = Reassembler::new();
        let first = split(50, &world(1000)).unwrap();
        assert_eq!(
            reassembler.push(&Chunk::parse(&first[0]).unwrap()),
            Pushed::Stored
        );
        let body = world(900);
        let second = split(50, &body).unwrap();
        assert_eq!(
            reassembler.push(&Chunk::parse(&second[1]).unwrap()),
            Pushed::Stored
        );
        assert_eq!(reassembler.missing(50), Some(480));
        assert_eq!(
            reassembler.push(&Chunk::parse(&second[0]).unwrap()),
            Pushed::Complete(Upload {
                item_time: 50,
                body
            })
        );
    }

    #[test]
    fn only_a_few_unfinished_uploads_are_kept() {
        let mut reassembler = Reassembler::new();
        for time in 0..10u32 {
            let chunks = split(time * 50, &world(1000)).unwrap();
            reassembler.push(&Chunk::parse(&chunks[0]).unwrap());
        }
        assert_eq!(reassembler.missing(0), None, "the oldest went");
        assert_eq!(reassembler.missing(450), Some(520));
        assert_eq!(reassembler.partial.len(), MAX_PARTIAL);
        reassembler.clear();
        assert!(!reassembler.is_receiving());
    }
}
