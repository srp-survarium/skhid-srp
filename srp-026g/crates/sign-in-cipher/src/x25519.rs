//! X25519 (RFC 7748): the Montgomery ladder over 16 limbs of 16 bits, after
//! TweetNaCl's `crypto_scalarmult`. Every step is the same for every scalar.

type Field = [i64; 16];

const A24: Field = [0xDB41, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

/// The base point, u = 9.
pub const BASE: [u8; 32] = {
    let mut base = [0; 32];
    base[0] = 9;
    base
};

fn carry(o: &mut Field) {
    for i in 0..16 {
        o[i] += 1 << 16;
        let c = o[i] >> 16;
        if i < 15 {
            o[i + 1] += c - 1;
        } else {
            o[0] += 38 * (c - 1);
        }
        o[i] -= c << 16;
    }
}

/// Swaps p and q when `bit` is 1, in the same time when it is 0.
fn select(p: &mut Field, q: &mut Field, bit: i64) {
    let mask = !(bit - 1);
    for (p, q) in p.iter_mut().zip(q) {
        let t = mask & (*p ^ *q);
        *p ^= t;
        *q ^= t;
    }
}

fn pack(n: &Field) -> [u8; 32] {
    let mut t = *n;
    carry(&mut t);
    carry(&mut t);
    carry(&mut t);
    for _ in 0..2 {
        let mut m: Field = [0; 16];
        m[0] = t[0] - 0xffed;
        for i in 1..15 {
            m[i] = t[i] - 0xffff - ((m[i - 1] >> 16) & 1);
            m[i - 1] &= 0xffff;
        }
        m[15] = t[15] - 0x7fff - ((m[14] >> 16) & 1);
        let borrow = (m[15] >> 16) & 1;
        m[14] &= 0xffff;
        select(&mut t, &mut m, 1 - borrow);
    }
    let mut out = [0; 32];
    for (bytes, limb) in out.chunks_exact_mut(2).zip(t) {
        bytes.copy_from_slice(&(limb as u16).to_le_bytes());
    }
    out
}

fn unpack(bytes: &[u8; 32]) -> Field {
    let mut out: Field = [0; 16];
    for (limb, pair) in out.iter_mut().zip(bytes.chunks_exact(2)) {
        *limb = i64::from(u16::from_le_bytes([pair[0], pair[1]]));
    }
    out[15] &= 0x7fff;
    out
}

fn add(a: &Field, b: &Field) -> Field {
    core::array::from_fn(|i| a[i] + b[i])
}

fn sub(a: &Field, b: &Field) -> Field {
    core::array::from_fn(|i| a[i] - b[i])
}

fn mul(a: &Field, b: &Field) -> Field {
    let mut t = [0_i64; 31];
    for i in 0..16 {
        for j in 0..16 {
            t[i + j] += a[i] * b[j];
        }
    }
    for i in 0..15 {
        t[i] += 38 * t[i + 16];
    }
    let mut out: Field = core::array::from_fn(|i| t[i]);
    carry(&mut out);
    carry(&mut out);
    out
}

fn square(a: &Field) -> Field {
    mul(a, a)
}

fn invert(i: &Field) -> Field {
    let mut c = *i;
    for a in (0..=253).rev() {
        c = square(&c);
        if a != 2 && a != 4 {
            c = mul(&c, i);
        }
    }
    c
}

/// `scalar` (clamped here) times the point with u-coordinate `point`.
pub fn scalar_mult(scalar: &[u8; 32], point: &[u8; 32]) -> [u8; 32] {
    let mut z = *scalar;
    z[31] = (z[31] & 127) | 64;
    z[0] &= 248;
    let x = unpack(point);
    let (mut a, mut b, mut c, mut d): (Field, Field, Field, Field) = ([0; 16], x, [0; 16], [0; 16]);
    a[0] = 1;
    d[0] = 1;
    for i in (0..=254_usize).rev() {
        let bit = i64::from((z[i >> 3] >> (i & 7)) & 1);
        select(&mut a, &mut b, bit);
        select(&mut c, &mut d, bit);
        let mut e = add(&a, &c);
        a = sub(&a, &c);
        c = add(&b, &d);
        b = sub(&b, &d);
        d = square(&e);
        let f = square(&a);
        a = mul(&c, &a);
        c = mul(&b, &e);
        e = add(&a, &c);
        a = sub(&a, &c);
        b = square(&a);
        c = sub(&d, &f);
        a = mul(&c, &A24);
        a = add(&a, &d);
        c = mul(&c, &a);
        a = mul(&d, &f);
        d = mul(&b, &x);
        b = square(&e);
        select(&mut a, &mut b, bit);
        select(&mut c, &mut d, bit);
    }
    pack(&mul(&a, &invert(&c)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::hex32;

    /// RFC 7748 section 5.2, both vectors.
    #[test]
    fn scalar_mult_of_the_rfc() {
        assert_eq!(
            scalar_mult(
                &hex32("a546e36bf0527c9d3b16154b82465edd62144c0ac1fc5a18506a2244ba449ac4"),
                &hex32("e6db6867583030db3594c1a424b15f7c726624ec26b3353b10a903a6d0ab1c4c"),
            ),
            hex32("c3da55379de9c6908e94ea4df28d084f32eccf03491c71f754b4075577a28552")
        );
        assert_eq!(
            scalar_mult(
                &hex32("4b66e9d4d1b4673c5ad22691957d6af5c11b6421e0ea01d42ca4169e7918ba0d"),
                &hex32("e5210f12786811d3f4b7959d0538ae2c31dbe7106fc03c3efc4cd549c715a493"),
            ),
            hex32("95cbde9476e8907d7aade45cb4b873f88b595a68799fa152e6f8f7647aac7957")
        );
    }

    /// RFC 7748 section 5.2: the base point through 1 and 1000 iterations.
    #[test]
    fn iterated_scalar_mult_of_the_rfc() {
        let (mut k, mut u) = (BASE, BASE);
        for round in 1..=1000 {
            let next = scalar_mult(&k, &u);
            u = k;
            k = next;
            if round == 1 {
                assert_eq!(
                    k,
                    hex32("422c8e7a6227d7bca1350b3e2bb7279f7897b87bb6854b783c60e80311ae3079")
                );
            }
        }
        assert_eq!(
            k,
            hex32("684cf59ba83309552800ef566f2f4d3c1c3887c49360e3875f2eb94d99532c51")
        );
    }

    /// RFC 7748 section 6.1: both public keys and the shared secret.
    #[test]
    fn key_exchange_of_the_rfc() {
        let alice = hex32("77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a");
        let bob = hex32("5dab087e624a8a4b79e17f8b83800ee66f3bb1292618b6fd1c2f8b27ff88e0eb");
        let alice_public = scalar_mult(&alice, &BASE);
        let bob_public = scalar_mult(&bob, &BASE);
        assert_eq!(
            alice_public,
            hex32("8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a")
        );
        assert_eq!(
            bob_public,
            hex32("de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f")
        );
        let shared = hex32("4a5d9d5ba4ce2de1728e3bf480350f25e07e21c947d19e3376f09b3c1e161742");
        assert_eq!(scalar_mult(&alice, &bob_public), shared);
        assert_eq!(scalar_mult(&bob, &alice_public), shared);
    }
}
