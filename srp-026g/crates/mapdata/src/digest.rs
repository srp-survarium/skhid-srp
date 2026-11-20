//! A 64-bit digest of built structures: the proof that a world read from
//! a file, shared by matches or stored more compactly is the world a
//! build gives, so the bots decide exactly as before
//! (`docs/notes/player_bots_mapdata.md` §6).
//!
//! Floats go in by their bits. A digest covers what a structure answers,
//! not how it is stored: [`crate::WalkGrid::digest`] takes each floor's
//! links as (target, kind), so a packed grid digests as the plain one.

/// Word by word: xor, multiply by an odd constant, fold the high bits
/// down. Not cryptographic: it catches change, not tampering.
#[derive(Clone, Copy, Debug)]
pub struct Digest(u64);

impl Default for Digest {
    fn default() -> Self {
        Self::new()
    }
}

impl Digest {
    pub fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }

    /// A digest started from `seed`: another function of the same words than [`Self::new`]'s (a second lane of a wider digest).
    pub fn seeded(seed: u64) -> Self {
        let mut digest = Self::new();
        digest.u64(seed);
        digest
    }

    pub fn u64(&mut self, value: u64) {
        let mixed = (self.0 ^ value).wrapping_mul(0x9e37_79b9_7f4a_7c15);
        self.0 = mixed ^ (mixed >> 29);
    }

    pub fn u32(&mut self, value: u32) {
        self.u64(u64::from(value));
    }

    pub fn f32(&mut self, value: f32) {
        self.u32(value.to_bits());
    }

    /// Bytes, eight at a time (the last word padded), after their length.
    pub fn bytes(&mut self, bytes: &[u8]) {
        self.len(bytes.len());
        let (words, rest) = bytes.as_chunks::<8>();
        for word in words {
            self.u64(u64::from_le_bytes(*word));
        }
        let mut last = [0u8; 8];
        last[..rest.len()].copy_from_slice(rest);
        self.u64(u64::from_le_bytes(last));
    }

    /// A length first, so that two arrays end where they did.
    pub fn len(&mut self, len: usize) {
        self.u64(len as u64);
    }

    pub fn finish(self) -> u64 {
        self.0
    }
}

/// [incremental grids] A 128-bit digest: two [`Digest`] lanes of other seeds over a byte stream, eight bytes a word whatever the
/// writes' sizes, the count of bytes last.
#[derive(Clone)]
pub struct Digest128 {
    lanes: [Digest; 2],
    word: [u8; 8],
    filled: usize,
    count: u64,
}

impl Default for Digest128 {
    fn default() -> Self {
        Self {
            lanes: [Digest::seeded(0x5352_5047_5249_4431), Digest::seeded(0x5352_5047_5249_4432)],
            word: [0; 8],
            filled: 0,
            count: 0,
        }
    }
}

impl Digest128 {
    pub fn bytes(&mut self, bytes: &[u8]) {
        self.count += bytes.len() as u64;
        for &byte in bytes {
            self.word[self.filled] = byte;
            self.filled += 1;
            if self.filled == 8 {
                self.push(u64::from_le_bytes(self.word));
                self.filled = 0;
            }
        }
    }

    fn push(&mut self, word: u64) {
        self.lanes[0].u64(word);
        // The second lane's words turned, so the lanes are not one function of the stream twice.
        self.lanes[1].u64(word.rotate_left(29) ^ 0xa5a5_a5a5_a5a5_a5a5);
    }

    /// A word as its eight bytes.
    pub fn u64(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }

    pub fn u32(&mut self, value: u32) {
        self.bytes(&value.to_le_bytes());
    }

    pub fn u128(&mut self, value: u128) {
        self.bytes(&value.to_le_bytes());
    }

    /// A string or bytes after their length: two end where they did.
    pub fn str(&mut self, value: &[u8]) {
        self.u64(value.len() as u64);
        self.bytes(value);
    }

    pub fn finish(mut self) -> u128 {
        if self.filled > 0 {
            self.word[self.filled..].fill(0);
            self.push(u64::from_le_bytes(self.word));
        }
        let count = self.count;
        self.push(count);
        (u128::from(self.lanes[0].finish()) << 64) | u128::from(self.lanes[1].finish())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_and_every_bit_count() {
        let digest = |values: &[u32]| {
            let mut digest = Digest::new();
            values.iter().for_each(|&value| digest.u32(value));
            digest.finish()
        };
        assert_eq!(digest(&[1, 2]), digest(&[1, 2]));
        assert_ne!(digest(&[1, 2]), digest(&[2, 1]));
        assert_ne!(digest(&[1, 2]), digest(&[1, 3]));
        assert_ne!(digest(&[0]), digest(&[]));
        let mut a = Digest::new();
        a.f32(0.0);
        let mut b = Digest::new();
        b.f32(-0.0);
        assert_ne!(a.finish(), b.finish());
    }
}
