//! SHA-256 (FIPS 180-4), HMAC (RFC 2104) and HKDF (RFC 5869) over it.

const K: [u32; 64] = [
    0x428a_2f98,
    0x7137_4491,
    0xb5c0_fbcf,
    0xe9b5_dba5,
    0x3956_c25b,
    0x59f1_11f1,
    0x923f_82a4,
    0xab1c_5ed5,
    0xd807_aa98,
    0x1283_5b01,
    0x2431_85be,
    0x550c_7dc3,
    0x72be_5d74,
    0x80de_b1fe,
    0x9bdc_06a7,
    0xc19b_f174,
    0xe49b_69c1,
    0xefbe_4786,
    0x0fc1_9dc6,
    0x240c_a1cc,
    0x2de9_2c6f,
    0x4a74_84aa,
    0x5cb0_a9dc,
    0x76f9_88da,
    0x983e_5152,
    0xa831_c66d,
    0xb003_27c8,
    0xbf59_7fc7,
    0xc6e0_0bf3,
    0xd5a7_9147,
    0x06ca_6351,
    0x1429_2967,
    0x27b7_0a85,
    0x2e1b_2138,
    0x4d2c_6dfc,
    0x5338_0d13,
    0x650a_7354,
    0x766a_0abb,
    0x81c2_c92e,
    0x9272_2c85,
    0xa2bf_e8a1,
    0xa81a_664b,
    0xc24b_8b70,
    0xc76c_51a3,
    0xd192_e819,
    0xd699_0624,
    0xf40e_3585,
    0x106a_a070,
    0x19a4_c116,
    0x1e37_6c08,
    0x2748_774c,
    0x34b0_bcb5,
    0x391c_0cb3,
    0x4ed8_aa4a,
    0x5b9c_ca4f,
    0x682e_6ff3,
    0x748f_82ee,
    0x78a5_636f,
    0x84c8_7814,
    0x8cc7_0208,
    0x90be_fffa,
    0xa450_6ceb,
    0xbef9_a3f7,
    0xc671_78f2,
];

pub const DIGEST: usize = 32;
const BLOCK: usize = 64;

pub struct Sha256 {
    state: [u32; 8],
    block: [u8; BLOCK],
    filled: usize,
    length: u64,
}

impl Sha256 {
    pub const fn new() -> Self {
        Self {
            state: [
                0x6a09_e667,
                0xbb67_ae85,
                0x3c6e_f372,
                0xa54f_f53a,
                0x510e_527f,
                0x9b05_688c,
                0x1f83_d9ab,
                0x5be0_cd19,
            ],
            block: [0; BLOCK],
            filled: 0,
            length: 0,
        }
    }

    pub fn update(&mut self, mut bytes: &[u8]) {
        self.length = self.length.wrapping_add(bytes.len() as u64);
        while !bytes.is_empty() {
            let take = bytes.len().min(BLOCK - self.filled);
            self.block[self.filled..self.filled + take].copy_from_slice(&bytes[..take]);
            self.filled += take;
            bytes = &bytes[take..];
            if self.filled == BLOCK {
                self.compress();
                self.filled = 0;
            }
        }
    }

    pub fn finish(mut self) -> [u8; DIGEST] {
        let bits = self.length.wrapping_mul(8);
        self.update(&[0x80]);
        while self.filled != BLOCK - 8 {
            self.update(&[0]);
        }
        self.update(&bits.to_be_bytes());
        let mut digest = [0; DIGEST];
        for (out, word) in digest.chunks_exact_mut(4).zip(self.state) {
            out.copy_from_slice(&word.to_be_bytes());
        }
        digest
    }

    fn compress(&mut self) {
        let mut w = [0_u32; 64];
        for (word, bytes) in w.iter_mut().zip(self.block.chunks_exact(4)) {
            *word = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = self.state;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choose = (e & f) ^ (!e & g);
            let t1 = h
                .wrapping_add(s1)
                .wrapping_add(choose)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(majority);
            (h, g, f, e, d, c, b, a) = (g, f, e, d.wrapping_add(t1), c, b, a, t1.wrapping_add(t2));
        }
        for (state, add) in self.state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *state = state.wrapping_add(add);
        }
    }
}

/// HMAC-SHA-256 of the parts, one after another.
pub fn hmac(key: &[u8], parts: &[&[u8]]) -> [u8; DIGEST] {
    let mut block = [0_u8; BLOCK];
    if key.len() > BLOCK {
        let mut hash = Sha256::new();
        hash.update(key);
        block[..DIGEST].copy_from_slice(&hash.finish());
    } else {
        block[..key.len()].copy_from_slice(key);
    }
    let mut inner = Sha256::new();
    inner.update(&block.map(|byte| byte ^ 0x36));
    for part in parts {
        inner.update(part);
    }
    let mut outer = Sha256::new();
    outer.update(&block.map(|byte| byte ^ 0x5c));
    outer.update(&inner.finish());
    outer.finish()
}

/// HKDF-SHA-256: 64 bytes (two blocks) of key material.
pub fn hkdf64(salt: &[u8], secret: &[u8], info: &[u8]) -> [u8; 64] {
    let key = hmac(salt, &[secret]);
    let first = hmac(&key, &[info, &[1]]);
    let second = hmac(&key, &[&first, info, &[2]]);
    let mut out = [0; 64];
    out[..DIGEST].copy_from_slice(&first);
    out[DIGEST..].copy_from_slice(&second);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::hex;

    #[test]
    fn sha256_of_abc_and_of_two_blocks() {
        let mut hash = Sha256::new();
        hash.update(b"abc");
        assert_eq!(
            hash.finish()[..],
            hex("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        );
        let mut hash = Sha256::new();
        hash.update(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq");
        assert_eq!(
            hash.finish()[..],
            hex("248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1")
        );
    }

    /// RFC 4231, test case 2.
    #[test]
    fn hmac_of_the_rfc() {
        assert_eq!(
            hmac(b"Jefe", &[b"what do ya want ", b"for nothing?"])[..],
            hex("5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843")
        );
    }

    /// RFC 5869, test case 1, lengthened to 64 bytes by the same expansion
    /// (its first 42 bytes are the RFC's).
    #[test]
    fn hkdf_of_the_rfc() {
        let out = hkdf64(
            &hex("000102030405060708090a0b0c"),
            &hex("0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b"),
            &hex("f0f1f2f3f4f5f6f7f8f9"),
        );
        assert_eq!(
            out[..42],
            hex(
                "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865"
            )
        );
    }
}
