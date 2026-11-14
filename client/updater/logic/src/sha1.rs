//! SHA-1, for the v1 info hash alone: the name of the torrent's swarm at
//! the trackers and in the peers' handshake. Nothing is trusted by it; the
//! torrent and every piece are checked by SHA-256 (`crate::torrent`).

pub fn digest(data: &[u8]) -> [u8; 20] {
    let mut state: [u32; 5] = [0x6745_2301, 0xEFCD_AB89, 0x98BA_DCFE, 0x1032_5476, 0xC3D2_E1F0];
    let bits = (data.len() as u64).wrapping_mul(8);
    let mut tail = [0_u8; 128];
    let full = data.len() / 64 * 64;
    let rest = &data[full..];
    tail[..rest.len()].copy_from_slice(rest);
    tail[rest.len()] = 0x80;
    let tail_len = if rest.len() < 56 { 64 } else { 128 };
    tail[tail_len - 8..tail_len].copy_from_slice(&bits.to_be_bytes());
    for block in data[..full].chunks_exact(64).chain(tail[..tail_len].chunks_exact(64)) {
        let mut w = [0_u32; 80];
        for (i, word) in block.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = state;
        for (i, word) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5A82_7999),
                20..=39 => (b ^ c ^ d, 0x6ED9_EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1B_BCDC),
                _ => (b ^ c ^ d, 0xCA62_C1D6),
            };
            let next = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(*word);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = next;
        }
        for (value, add) in state.iter_mut().zip([a, b, c, d, e]) {
            *value = value.wrapping_add(add);
        }
    }
    let mut out = [0_u8; 20];
    for (bytes, value) in out.chunks_exact_mut(4).zip(state) {
        bytes.copy_from_slice(&value.to_be_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::digest;

    fn hex(bytes: [u8; 20]) -> alloc::string::String {
        bytes.iter().map(|b| alloc::format!("{b:02x}")).collect()
    }

    #[test]
    fn known_digests() {
        assert_eq!(hex(digest(b"")), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        assert_eq!(hex(digest(b"abc")), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(
            hex(digest(
                b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"
            )),
            "84983e441c3bd26ebaae4aa1f95129e5e54670f1"
        );
        assert_eq!(
            hex(digest(&[b'a'; 1000])),
            "291e9a6c66994949b57ba5e650361e98fc36b1ba"
        );
    }
}
