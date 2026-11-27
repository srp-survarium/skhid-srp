//! TCP framing of the 0.69d login connection (`docs/notes/login.md`; PROVEN with the real client, which signs in
//! with both forms): `u8 len` (1..=255) + payload, or `00` + `u32 LE len` + payload.
//! payload[0] = message type. Both directions; the server always answers in the 5-byte form
//! (PROVEN: the client accepts it).

use std::fmt;

/// Largest payload accepted (the real messages are under 500 bytes).
pub const MAX_PAYLOAD: usize = 1 << 20;

#[derive(Debug, PartialEq, Eq)]
pub enum FrameError {
    TooLarge(usize),
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FrameError::TooLarge(n) => write!(f, "frame of {n} bytes is too large"),
        }
    }
}

impl std::error::Error for FrameError {}

/// Wrap `payload` in the 5-byte frame form.
pub fn frame(payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(5 + payload.len());
    out.push(0);
    out.extend((payload.len() as u32).to_le_bytes());
    out.extend(payload);
    out
}

/// Reassembles frames from a TCP byte stream.
#[derive(Default)]
pub struct FrameReader {
    buf: Vec<u8>,
}

impl FrameReader {
    pub fn push(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    /// The next complete frame's payload, `Ok(None)` when more bytes are needed.
    pub fn next_frame(&mut self) -> Result<Option<Vec<u8>>, FrameError> {
        let Some(&flag) = self.buf.first() else { return Ok(None) };
        let (head, len) = if flag != 0 {
            (1, usize::from(flag))
        } else if self.buf.len() >= 5 {
            (5, u32::from_le_bytes([self.buf[1], self.buf[2], self.buf[3], self.buf[4]]) as usize)
        } else {
            return Ok(None);
        };
        if len > MAX_PAYLOAD {
            return Err(FrameError::TooLarge(len));
        }
        if self.buf.len() < head + len {
            return Ok(None);
        }
        let payload = self.buf[head..head + len].to_vec();
        self.buf.drain(..head + len);
        Ok(Some(payload))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_forms_and_split_reads() {
        let mut r = FrameReader::default();
        // short form, then long form, fed byte by byte
        let mut bytes = vec![3, 1, 2, 3];
        bytes.extend(frame(&[9, 8]));
        for b in &bytes {
            r.push(&[*b]);
        }
        assert_eq!(r.next_frame().unwrap(), Some(vec![1, 2, 3]));
        assert_eq!(r.next_frame().unwrap(), Some(vec![9, 8]));
        assert_eq!(r.next_frame().unwrap(), None);
    }

    #[test]
    fn partial_and_oversized() {
        let mut r = FrameReader::default();
        r.push(&[0, 5, 0]);
        assert_eq!(r.next_frame().unwrap(), None);
        let mut r = FrameReader::default();
        r.push(&[0, 0xFF, 0xFF, 0xFF, 0x7F]);
        assert!(matches!(r.next_frame(), Err(FrameError::TooLarge(_))));
    }

    #[test]
    fn golden_success_frame() {
        assert_eq!(frame(&[0x0B, 0]).len(), 7);
        assert_eq!(&frame(&[0x0B, 0])[..5], &[0, 2, 0, 0, 0]);
    }
}
