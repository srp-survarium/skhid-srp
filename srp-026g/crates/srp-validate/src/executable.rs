//! The running executable only: a hash offer, then server-requested windows
//! of file bytes on the match connection. No client-supplied path is accepted.
//! E0 upload carriers have an impossible world-chunk prefix. E4 replies are
//! consumed before native options parsing. All integers are little endian.

pub const MAGIC: [u8; 8] = [255, 255, 255, 255, 255, 255, 4, 0];
pub const REPLY_MAGIC: [u8; 4] = [255, b'E', b'X', 1];
pub const CHUNK: usize = 400;
pub const WINDOW: u32 = 16 * CHUNK as u32;
pub const MAX_FILE: u32 = 16 * 1024 * 1024;
pub const MAX_PACKET: usize = 480;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Identity {
    size: u32,
    pub sha256: [u8; 32],
}
impl Identity {
    pub fn new(size: u32, sha256: [u8; 32]) -> Option<Self> {
        (size > 0 && size <= MAX_FILE).then_some(Self { size, sha256 })
    }
    pub fn size(self) -> u32 { self.size }
    pub fn chunk_len(self, offset: u32) -> Option<usize> {
        (offset < self.size && offset.is_multiple_of(CHUNK as u32))
            .then(|| (self.size.saturating_sub(offset)).min(CHUNK as u32) as usize)
    }
    pub fn window_end(self, offset: u32) -> Option<u32> {
        (offset < self.size && offset.is_multiple_of(WINDOW)).then(|| (offset.saturating_add(WINDOW)).min(self.size))
    }
}
impl core::fmt::Display for Identity {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for b in self.sha256 { write!(f, "{b:02x}")?; }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Upload<'a> {
    Offer(Identity),
    Data { sha256: [u8; 32], offset: u32, bytes: &'a [u8] },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Carrier<'a> {
    pub match_id: u32,
    pub upload: Upload<'a>,
}
fn word(bytes: &[u8]) -> Option<u32> { Some(u32::from_le_bytes(bytes.try_into().ok()?)) }

/// A length-delimited carrier, including when followed by another message.
pub fn carrier_len(bytes: &[u8]) -> Option<usize> {
    if !bytes.starts_with(&MAGIC) { return None; }
    let size = usize::from(u16::from_le_bytes(bytes.get(8..10)?.try_into().ok()?));
    ((15..=MAX_PACKET).contains(&size) && bytes.len() >= size).then_some(size)
}
impl<'a> Carrier<'a> {
    pub fn parse(bytes: &'a [u8]) -> Option<Self> {
        if carrier_len(bytes)? != bytes.len() { return None; }
        let match_id = word(bytes.get(10..14)?)?;
        let hash: [u8; 32] = bytes.get(15..47)?.try_into().ok()?;
        let value = word(bytes.get(47..51)?)?;
        let rest = bytes.get(51..)?;
        let upload = match *bytes.get(14)? {
            0 if rest.is_empty() => Upload::Offer(Identity::new(value, hash)?),
            1 if value < MAX_FILE && value.is_multiple_of(CHUNK as u32) && !rest.is_empty() && rest.len() <= CHUNK =>
                Upload::Data { sha256: hash, offset: value, bytes: rest },
            _ => return None,
        };
        Some(Self { match_id, upload })
    }
    pub fn encode(self) -> Option<([u8; MAX_PACKET], usize)> {
        let mut out = [0; MAX_PACKET];
        out[..8].copy_from_slice(&MAGIC);
        out[10..14].copy_from_slice(&self.match_id.to_le_bytes());
        let (kind, hash, value, data) = match self.upload {
            Upload::Offer(identity) => (0, identity.sha256, identity.size, &[][..]),
            Upload::Data { sha256, offset, bytes } => (1, sha256, offset, bytes),
        };
        let len = 51usize.checked_add(data.len())?;
        out[8..10].copy_from_slice(&(len as u16).to_le_bytes());
        out[14] = kind;
        out[15..47].copy_from_slice(&hash);
        out[47..51].copy_from_slice(&value.to_le_bytes());
        out.get_mut(51..len)?.copy_from_slice(data);
        Carrier::parse(out.get(..len)?)?;
        Some((out, len))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer { Need(u32), Stored, Refused }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reply { pub match_id: u32, pub sha256: [u8; 32], pub answer: Answer }
impl Reply {
    pub const SIZE: usize = 45;
    pub fn encode(self) -> [u8; Self::SIZE] {
        let mut out = [0; Self::SIZE];
        out[..4].copy_from_slice(&REPLY_MAGIC);
        out[4..8].copy_from_slice(&self.match_id.to_le_bytes());
        out[8..40].copy_from_slice(&self.sha256);
        let (kind, offset) = match self.answer { Answer::Need(n) => (0, n), Answer::Stored => (1, 0), Answer::Refused => (2, 0) };
        out[40] = kind;
        out[41..45].copy_from_slice(&offset.to_le_bytes());
        out
    }
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != Self::SIZE || !bytes.starts_with(&REPLY_MAGIC) { return None; }
        let offset = word(bytes.get(41..45)?)?;
        let answer = match (*bytes.get(40)?, offset) {
            (0, n) if n < MAX_FILE && n.is_multiple_of(WINDOW) => Answer::Need(n),
            (1, 0) => Answer::Stored,
            (2, 0) => Answer::Refused,
            _ => return None,
        };
        Some(Self { match_id: word(bytes.get(4..8)?)?, sha256: bytes.get(8..40)?.try_into().ok()?, answer })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn carriers_are_bounded_and_unambiguous_in_joined_datagrams() {
        let identity = Identity::new(MAX_FILE, [0xa5; 32]).unwrap();
        for upload in [Upload::Offer(identity), Upload::Data { sha256: identity.sha256, offset: WINDOW, bytes: &[9; CHUNK] }] {
            let carrier = Carrier { match_id: 42, upload };
            let (bytes, len) = carrier.encode().unwrap();
            assert_eq!(Carrier::parse(&bytes[..len]), Some(carrier));
            assert!((0..len).all(|n| Carrier::parse(&bytes[..n]).is_none()));
            let mut joined = bytes[..len].to_vec(); joined.extend([0xe1, 0, 0]);
            assert_eq!(carrier_len(&joined), Some(len));
            assert!(Carrier::parse(&joined).is_none());
            assert!(crate::report::Carrier::parse(&bytes[..len]).is_none());
        }
        assert!(Identity::new(0, [0; 32]).is_none());
        assert!(Identity::new(MAX_FILE + 1, [0; 32]).is_none());
        assert_eq!(identity.chunk_len(MAX_FILE - MAX_FILE % CHUNK as u32), Some(MAX_FILE as usize % CHUNK));
        for answer in [Answer::Need(WINDOW), Answer::Stored, Answer::Refused] {
            let reply = Reply { match_id: 42, sha256: [3; 32], answer };
            assert_eq!(Reply::parse(&reply.encode()), Some(reply));
        }
        assert!(Reply::parse(&Reply { match_id: 1, sha256: [0; 32], answer: Answer::Need(1) }.encode()).is_none());
    }
}
