//! The password digest the 0.69d client sends, computed on the server for the Steam import panel (`04` sub 2 carries
//! the typed password in clear; see [`crate::handshake`]).
//!
//! PROVEN from the exe: the digest is SHA-1 of the password bytes as typed (at most 47 bytes kept, no text
//! conversion), written as 40 lowercase hex digits; the e-mail field keeps at most 99 bytes. Addresses: form
//! `sign_in_button_clicked` 0x200126F51..0x200127010 (e-mail field 0x200126F9B), hash 0x20011E6EE -> 0x200122BD0,
//! hex format 0x20011E730 (format string 0x2009575CC).

/// Longest password the normal form keeps (bytes).
pub const FORM_PASSWORD_MAX: usize = 47;
/// Longest e-mail (account name) the normal form keeps (bytes).
pub const FORM_EMAIL_MAX: usize = 99;

/// SHA-1 (FIPS 180-4).
pub fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
    let mut msg = data.to_vec();
    let bits = (data.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend(bits.to_be_bytes());
    for block in msg.chunks_exact(64) {
        let mut w = [0u32; 80];
        for (i, c) in block.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes([c[0], c[1], c[2], c[3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);
        for (i, wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let t = a.rotate_left(5).wrapping_add(f).wrapping_add(e).wrapping_add(k).wrapping_add(*wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = t;
        }
        for (x, y) in h.iter_mut().zip([a, b, c, d, e]) {
            *x = x.wrapping_add(y);
        }
    }
    let mut out = [0u8; 20];
    for (i, x) in h.iter().enumerate() {
        out[4 * i..4 * i + 4].copy_from_slice(&x.to_be_bytes());
    }
    out
}

/// SHA-1 as 40 lowercase hex digits (the client's `%08x` x 5).
pub fn sha1_hex(data: &[u8]) -> String {
    sha1(data).iter().map(|b| format!("{b:02x}")).collect()
}

/// What the normal form keeps of a text field: the bytes up to the first zero, cut to `max` bytes.
pub fn form_field(text: &[u8], max: usize) -> &[u8] {
    let end = text.iter().position(|&b| b == 0).unwrap_or(text.len()).min(max);
    &text[..end]
}

/// The digest the normal client sends for this typed password (password type 1).
pub fn client_password_digest(password: &[u8]) -> String {
    sha1_hex(form_field(password, FORM_PASSWORD_MAX))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha1_known_vectors() {
        assert_eq!(sha1_hex(b""), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        assert_eq!(sha1_hex(b"abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(
            sha1_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "84983e441c3bd26ebaae4aa1f95129e5e54670f1"
        );
        assert_eq!(sha1_hex(&[b'a'; 1_000_000]), "34aa973cd4c4daa4f61eeb2bdbad27316534016f");
        // the 55 / 56 / 64-byte padding edges
        assert_eq!(sha1_hex(&[b'a'; 55]), "c1c8bbdc22796e28c0e15163d20899b65621d65a");
        assert_eq!(sha1_hex(&[b'a'; 64]), "0098ba824b5c16427bd7a1122a5a442a25ec644d");
    }

    #[test]
    fn digest_follows_the_form_field() {
        // lowercase hex, the bytes as typed (UTF-8 from the movie), no case change
        assert_eq!(client_password_digest(b"abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_ne!(client_password_digest(b"ABC"), client_password_digest(b"abc"));
        // 47 bytes kept: a longer password signs in with its first 47 bytes, as from the normal form
        let long = [b'x'; 60];
        assert_eq!(client_password_digest(&long), sha1_hex(&long[..47]));
        assert_eq!(client_password_digest(&long[..47]), client_password_digest(&long[..48]));
        assert_eq!(form_field(b"ab\0cd", 99), b"ab");
        assert_eq!(form_field(&[b'e'; 120], FORM_EMAIL_MAX).len(), 99);
    }
}
