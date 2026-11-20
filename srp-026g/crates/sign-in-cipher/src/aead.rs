//! ChaCha20-Poly1305 (RFC 8439), for the short messages of a sign-in: the
//! whole message is sealed or opened in one call.

pub const KEY: usize = 32;
pub const NONCE: usize = 12;
pub const TAG: usize = 16;

fn quarter(state: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize) {
    state[a] = state[a].wrapping_add(state[b]);
    state[d] = (state[d] ^ state[a]).rotate_left(16);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = (state[b] ^ state[c]).rotate_left(12);
    state[a] = state[a].wrapping_add(state[b]);
    state[d] = (state[d] ^ state[a]).rotate_left(8);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = (state[b] ^ state[c]).rotate_left(7);
}

fn block(key: &[u8; KEY], counter: u32, nonce: &[u8; NONCE]) -> [u8; 64] {
    let word = |bytes: &[u8]| u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    let mut start = [
        0x6170_7865,
        0x3320_646e,
        0x7962_2d32,
        0x6b20_6574,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        0,
        counter,
        0,
        0,
        0,
    ];
    for (state, bytes) in start[4..12].iter_mut().zip(key.chunks_exact(4)) {
        *state = word(bytes);
    }
    for (state, bytes) in start[13..].iter_mut().zip(nonce.chunks_exact(4)) {
        *state = word(bytes);
    }
    let mut state = start;
    for _ in 0..10 {
        quarter(&mut state, 0, 4, 8, 12);
        quarter(&mut state, 1, 5, 9, 13);
        quarter(&mut state, 2, 6, 10, 14);
        quarter(&mut state, 3, 7, 11, 15);
        quarter(&mut state, 0, 5, 10, 15);
        quarter(&mut state, 1, 6, 11, 12);
        quarter(&mut state, 2, 7, 8, 13);
        quarter(&mut state, 3, 4, 9, 14);
    }
    let mut out = [0; 64];
    for ((bytes, word), start) in out.chunks_exact_mut(4).zip(state).zip(start) {
        bytes.copy_from_slice(&word.wrapping_add(start).to_le_bytes());
    }
    out
}

/// XORs the key stream, from block 1 on, into `bytes`.
fn stream(key: &[u8; KEY], nonce: &[u8; NONCE], bytes: &mut [u8]) {
    for (counter, chunk) in (1_u32..).zip(bytes.chunks_mut(64)) {
        for (byte, key) in chunk.iter_mut().zip(block(key, counter, nonce)) {
            *byte ^= key;
        }
    }
}

/// Poly1305 over 17 limbs of 8 bits, after TweetNaCl's `crypto_onetimeauth`.
struct Poly1305 {
    r: [u32; 17],
    h: [u32; 17],
    pad: [u8; 16],
}

fn add1305(h: &mut [u32; 17], c: &[u32; 17]) {
    let mut carry = 0;
    for (h, c) in h.iter_mut().zip(c) {
        carry += *h + *c;
        *h = carry & 255;
        carry >>= 8;
    }
}

impl Poly1305 {
    fn new(key: &[u8; 32]) -> Self {
        let mut r = [0_u32; 17];
        for (limb, byte) in r.iter_mut().zip(&key[..16]) {
            *limb = u32::from(*byte);
        }
        for i in [3, 7, 11, 15] {
            r[i] &= 15;
        }
        for i in [4, 8, 12] {
            r[i] &= 252;
        }
        let mut pad = [0; 16];
        pad.copy_from_slice(&key[16..]);
        Self { r, h: [0; 17], pad }
    }

    /// One block of at most 16 bytes; a shorter one only as the last.
    fn block(&mut self, bytes: &[u8]) {
        let mut c = [0_u32; 17];
        for (limb, byte) in c.iter_mut().zip(bytes) {
            *limb = u32::from(*byte);
        }
        c[bytes.len()] = 1;
        add1305(&mut self.h, &c);
        let mut x = [0_u32; 17];
        for (i, x) in x.iter_mut().enumerate() {
            for j in 0..17 {
                *x += self.h[j]
                    * if j <= i {
                        self.r[i - j]
                    } else {
                        320 * self.r[i + 17 - j]
                    };
            }
        }
        self.h = x;
        let mut carry = 0;
        for h in &mut self.h[..16] {
            carry += *h;
            *h = carry & 255;
            carry >>= 8;
        }
        carry += self.h[16];
        self.h[16] = carry & 3;
        carry = 5 * (carry >> 2);
        for h in &mut self.h[..16] {
            carry += *h;
            *h = carry & 255;
            carry >>= 8;
        }
        self.h[16] += carry;
    }

    /// The bytes in blocks of 16, the last zero-padded to 16 (the AEAD's
    /// padding: its every block is whole).
    fn padded(&mut self, bytes: &[u8]) {
        for chunk in bytes.chunks(16) {
            let mut whole = [0; 16];
            whole[..chunk.len()].copy_from_slice(chunk);
            self.block(&whole);
        }
    }

    fn finish(mut self) -> [u8; TAG] {
        const MINUS_P: [u32; 17] = [5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 252];
        let g = self.h;
        add1305(&mut self.h, &MINUS_P);
        let mask = (self.h[16] >> 7).wrapping_neg();
        for (h, g) in self.h.iter_mut().zip(g) {
            *h ^= mask & (g ^ *h);
        }
        let mut c = [0_u32; 17];
        for (limb, byte) in c.iter_mut().zip(self.pad) {
            *limb = u32::from(byte);
        }
        add1305(&mut self.h, &c);
        core::array::from_fn(|i| self.h[i] as u8)
    }
}

fn tag(key: &[u8; KEY], nonce: &[u8; NONCE], aad: &[u8], sealed: &[u8]) -> [u8; TAG] {
    let first = block(key, 0, nonce);
    let mut poly = Poly1305::new(first[..32].try_into().unwrap_or(&[0; 32]));
    poly.padded(aad);
    poly.padded(sealed);
    let mut lengths = [0; 16];
    lengths[..8].copy_from_slice(&(aad.len() as u64).to_le_bytes());
    lengths[8..].copy_from_slice(&(sealed.len() as u64).to_le_bytes());
    poly.block(&lengths);
    poly.finish()
}

/// Encrypts `bytes` in place and returns their tag.
pub fn seal(key: &[u8; KEY], nonce: &[u8; NONCE], aad: &[u8], bytes: &mut [u8]) -> [u8; TAG] {
    stream(key, nonce, bytes);
    tag(key, nonce, aad, bytes)
}

/// Decrypts `bytes` in place when `expected` is their tag; else leaves them
/// sealed and says false. The tags are compared in the same time whatever
/// they are.
#[must_use]
pub fn open(
    key: &[u8; KEY],
    nonce: &[u8; NONCE],
    aad: &[u8],
    bytes: &mut [u8],
    expected: &[u8; TAG],
) -> bool {
    let actual = tag(key, nonce, aad, bytes);
    let difference = actual
        .iter()
        .zip(expected)
        .fold(0, |all, (a, b)| all | (a ^ b));
    if difference != 0 {
        return false;
    }
    stream(key, nonce, bytes);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{hex, hex32};

    const PLAIN: &[u8] = b"Ladies and Gentlemen of the class of '99: If I could offer you only one tip for the future, sunscreen would be it.";

    /// RFC 8439 section 2.8.2.
    #[test]
    fn seal_and_open_of_the_rfc() {
        let key = hex32("808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9f");
        let nonce: [u8; NONCE] = hex("070000004041424344454647").try_into().unwrap();
        let aad = hex("50515253c0c1c2c3c4c5c6c7");
        let mut bytes = PLAIN.to_vec();
        let tag = seal(&key, &nonce, &aad, &mut bytes);
        assert_eq!(
            bytes,
            hex(
                "d31a8d34648e60db7b86afbc53ef7ec2a4aded51296e08fea9e2b5a736ee62d63dbea45e8ca9671282fafb69da92728b1a71de0a9e060b2905d6a5b67ecd3b3692ddbd7f2d778b8c9803aee328091b58fab324e4fad675945585808b4831d7bc3ff4def08e4b7a9de576d26586cec64b6116"
            )
        );
        assert_eq!(tag[..], hex("1ae10b594f09e26a7e902ecbd0600691"));
        assert!(open(&key, &nonce, &aad, &mut bytes, &tag));
        assert_eq!(bytes, PLAIN);
    }

    #[test]
    fn a_changed_byte_anywhere_is_refused() {
        let key = [7; KEY];
        let nonce = [9; NONCE];
        let mut sealed = *b"the password";
        let tag = seal(&key, &nonce, b"aad", &mut sealed);
        for bit in 0..sealed.len() * 8 {
            let mut bytes = sealed;
            bytes[bit / 8] ^= 1 << (bit % 8);
            assert!(!open(&key, &nonce, b"aad", &mut bytes, &tag));
        }
        let mut wrong_tag = tag;
        wrong_tag[15] ^= 0x80;
        let mut bytes = sealed;
        assert!(!open(&key, &nonce, b"aad", &mut bytes, &wrong_tag));
        assert!(!open(&key, &nonce, b"aae", &mut bytes, &tag));
        assert!(open(&key, &nonce, b"aad", &mut bytes, &tag));
        assert_eq!(&bytes, b"the password");
    }
}
