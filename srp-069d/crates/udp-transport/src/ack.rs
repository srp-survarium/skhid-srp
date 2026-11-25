//! The 0.69d datagram header: seq, R, ack0 and the tail bytes
//! (PROVEN at 0x200755530).
//!
//! `byte0 = seq & 0xFF`, `byte1 = R & 0xFF`, `byte2 = ack0`. Header stream: ack0, then the datagram's
//! LAST byte, the one before it, ... A byte with bit 7 set means "one more follows"; the first byte with
//! bit 7 clear is the final one. Non-final byte: 7 ack bits (bit 6 first). Final byte: 4 ack bits
//! (bits 6..3), bit 2 = bit 8 of R, bit 1 = bit 8 of seq, bit 0 = F (1 = multi, 0 = single).
//! Stream bit m acks seq `(R - 1 - m) mod 512`; R itself is always acked.

use crate::seq::{MASK, NONE};
use std::collections::BTreeSet;
use std::fmt;

/// Tail bytes after ack0: at most 35 (256-bit mask, 7 bits a byte).
pub const MAX_TAIL: usize = 35;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HeaderError {
    TooShort,
    /// The tail walk found no final byte inside the datagram, or more than [`MAX_TAIL`] bytes.
    BadTail,
    /// `encode`: ack bit count is not `7n + 4` with n <= 35.
    BadBits(usize),
}

impl fmt::Display for HeaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HeaderError::TooShort => write!(f, "datagram shorter than the 3-byte header"),
            HeaderError::BadTail => write!(f, "ack tail has no final byte or is too long"),
            HeaderError::BadBits(n) => write!(f, "{n} ack bits is not 7n+4 (n <= 35)"),
        }
    }
}

impl std::error::Error for HeaderError {}

/// A decoded header. `bits` keeps the raw ack stream (length `7n + 4`, stream bit 0 first), so
/// `encode(decode(d)) == d` for any datagram.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
    pub seq: u16,
    pub r: u16,
    pub multi: bool,
    pub bits: Vec<bool>,
}

impl Header {
    /// Number of tail bytes (the final byte included; ack0 not).
    pub fn tail_len(&self) -> usize {
        (self.bits.len() - 4) / 7
    }

    /// Build a header that acks `got` (stream bit m <-> peer seq `(r - 1 - m) & 0x1FF`), `n` tail bytes.
    pub fn new(seq: u16, r: u16, multi: bool, got: &BTreeSet<u16>, n: usize) -> Header {
        let n = n.min(MAX_TAIL);
        let bits = (0..7 * n + 4)
            .map(|m| got.contains(&(r.wrapping_sub(1 + m as u16) & MASK)))
            .collect();
        Header { seq: seq & MASK, r: r & MASK, multi, bits }
    }

    /// The seqs this header acks: R (unless it is the "none yet" value) and every set stream bit.
    /// Seqs the sender never sent (ghost bits the client emits for its not-yet-valid history) are
    /// the caller's to ignore.
    pub fn acked(&self) -> Vec<u16> {
        if self.r == NONE {
            return Vec::new();
        }
        let mut out = vec![self.r];
        for (m, b) in self.bits.iter().enumerate() {
            if *b {
                out.push(self.r.wrapping_sub(1 + m as u16) & MASK);
            }
        }
        out
    }
}

/// Decode the header of `d`. Returns it and the end of the message area (`d.len()` minus the tail
/// bytes); the message area starts at byte 3.
pub fn decode(d: &[u8]) -> Result<(Header, usize), HeaderError> {
    if d.len() < 3 {
        return Err(HeaderError::TooShort);
    }
    let mut vals = vec![d[2]];
    if d[2] & 0x80 != 0 {
        let mut n = 0;
        loop {
            n += 1;
            // the message area must keep room for the 3 head bytes
            if n > MAX_TAIL || n > d.len() - 3 {
                return Err(HeaderError::BadTail);
            }
            let v = d[d.len() - n];
            vals.push(v);
            if v & 0x80 == 0 {
                break;
            }
        }
    }
    let final_byte = *vals.last().expect("ack0 is there");
    let mut bits = Vec::with_capacity(7 * vals.len());
    for v in &vals[..vals.len() - 1] {
        bits.extend((0..7).map(|j| (v >> (6 - j)) & 1 == 1));
    }
    bits.extend((0..4).map(|j| (final_byte >> (6 - j)) & 1 == 1));
    let seq = u16::from(d[0]) | (u16::from(final_byte >> 1) & 1) << 8;
    let r = u16::from(d[1]) | (u16::from(final_byte >> 2) & 1) << 8;
    let area_end = d.len() - (vals.len() - 1);
    Ok((Header { seq, r, multi: final_byte & 1 == 1, bits }, area_end))
}

/// Put the header around `body`: `seq r ack0 | body | tail`.
pub fn encode(h: &Header, body: &[u8]) -> Result<Vec<u8>, HeaderError> {
    if h.bits.len() < 4 || (h.bits.len() - 4) % 7 != 0 || (h.bits.len() - 4) / 7 > MAX_TAIL {
        return Err(HeaderError::BadBits(h.bits.len()));
    }
    let n = h.tail_len();
    let mut out = Vec::with_capacity(3 + body.len() + n);
    let mut stream = Vec::with_capacity(n + 1);
    for k in 0..n {
        let mut v = 0x80u8;
        for i in 0..7 {
            v |= u8::from(h.bits[7 * k + i]) << (6 - i);
        }
        stream.push(v);
    }
    let mut fin = 0u8;
    for i in 0..4 {
        fin |= u8::from(h.bits[7 * n + i]) << (6 - i);
    }
    fin |= (((h.r >> 8) & 1) as u8) << 2 | (((h.seq >> 8) & 1) as u8) << 1 | u8::from(h.multi);
    stream.push(fin);
    out.push((h.seq & 0xFF) as u8);
    out.push((h.r & 0xFF) as u8);
    out.push(stream[0]);
    out.extend_from_slice(body);
    // tail: t_n (final) first ... t_1 last
    if n > 0 {
        out.push(stream[n]);
        for k in (1..n).rev() {
            out.push(stream[k]);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(v: &[u16]) -> BTreeSet<u16> {
        v.iter().copied().collect()
    }

    #[test]
    fn live_samples() {
        // 00 ff 04 2e ...: seq 0, R = 0x1FF (none yet), single
        let (h, end) = decode(&hex("00ff042e0078563412ffff")).unwrap();
        assert_eq!((h.seq, h.r, h.multi, end), (0, 0x1FF, false, 11));
        assert!(h.acked().is_empty());
        // 03 14 78 fc: seq 3, R 0x14, acks 16..=20
        let (h, end) = decode(&hex("031478fc")).unwrap();
        assert_eq!((h.seq, h.r, h.multi, end), (3, 0x14, false, 4));
        let mut a = h.acked();
        a.sort();
        assert_eq!(a, vec![16, 17, 18, 19, 20]);
    }

    #[test]
    fn round_trip_widths() {
        for n in [0usize, 1, 3, 4, 35] {
            for (seq, r) in [(5u16, 5u16), (300, 0x1FE), (0x1FF, 0), (0, 0x100)] {
                let got = set(&[r.wrapping_sub(1) & MASK, r.wrapping_sub(5) & MASK, r]);
                let h = Header::new(seq, r, n % 2 == 1, &got, n);
                let d = encode(&h, b"\xfc").unwrap();
                let (back, end) = decode(&d).unwrap();
                assert_eq!(back, h, "n={n} seq={seq} r={r}");
                assert_eq!(&d[3..end], b"\xfc");
                assert_eq!(encode(&back, &d[3..end]).unwrap(), d);
            }
        }
    }

    #[test]
    fn recorded_server_datagram() {
        // a recorded server datagram: seq 0x2e R 0x0c, nothing acked in the window, body FC
        let d = hex("2e0cfffc008080fc");
        let (h, end) = decode(&d).unwrap();
        assert_eq!((h.seq, h.r, h.multi, h.tail_len()), (0x2e, 0x0c, false, 4));
        assert_eq!(&d[3..end], &[0xfc]);
        assert_eq!(encode(&h, &d[3..end]).unwrap(), d);
    }

    #[test]
    fn bad_tails() {
        assert_eq!(decode(&[1, 2]), Err(HeaderError::TooShort));
        // ack0 says "more bytes follow" but every following byte also has bit 7 set
        assert_eq!(decode(&[0, 0, 0x80, 0x80, 0x80]), Err(HeaderError::BadTail));
        assert_eq!(decode(&[0, 0, 0x80]), Err(HeaderError::BadTail));
        let h = Header { seq: 0, r: 0, multi: false, bits: vec![false; 5] };
        assert_eq!(encode(&h, &[]), Err(HeaderError::BadBits(5)));
    }

    #[test]
    fn bit_eight_flags() {
        let h = Header::new(0x100, 0x1FF, true, &BTreeSet::new(), 0);
        let d = encode(&h, &[0xFC]).unwrap();
        assert_eq!(d, vec![0x00, 0xFF, 0b0000_0111, 0xFC]);
    }

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect()
    }
}
