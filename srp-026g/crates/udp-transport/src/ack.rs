//! The ack field of a datagram (transport.md §2.1, §2.2): a 128-bit window
//! of received seqs sent as a 7-bit varint. The first byte (`ack0`) is header
//! byte 2; the other `n` bytes (0..=17) sit at the END of the datagram,
//! written in reverse, so the terminating byte is the datagram's last one.
//!
//! Bit 7 of an ack byte means "another ack byte follows". The terminating
//! byte (`ack0` when `n == 0`) has bit 7 clear and carries the single/multi
//! flag in bit 0 (1 = multi), so it holds 6 ack bits and every other byte 7.
//! With `b(k)` = "seq `rseq - k` was received":
//!
//! ```text
//! n == 0:  ack0 = F | b(1)<<6 | b(2)<<5 | ... | b(6)<<1
//! n >= 1:  ack0 = 0x80 | b(1)<<6 | ... | b(7)
//!          t_i  = 0x80 | b(7i+1)<<6 | ... | b(7i+7)        (1 <= i < n)
//!          t_n  = F | b(7n+1)<<6 | ... | b(7n+6)<<1
//! wire:    seq rseq ack0 <messages> t_n ... t_2 t_1
//! ```
//!
//! `rseq` itself is always implied. The reader (client 0x3FA0A0) counts the
//! valid bits including `rseq` as `7n + 7`.

/// The most tail bytes a datagram sent here carries: `7 * 17 + 6 = 125`
/// ack bits, which is also the most a client sends (its depth is capped
/// at 120).
pub const MAX_TAIL: usize = 17;
/// The most tail bytes taken from a datagram: one more, 127 ack bits. The
/// client's own reader (0x3FA157..0x3FA1C3) has no limit but the
/// datagram's length, and another server's datagrams with 18 were taken
/// by real clients.
pub const MAX_TAIL_READ: usize = 18;

/// A decoded ack field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Acks {
    /// Bit 127 = `rseq` (always set), bit `127 - k` = `rseq - k`.
    pub bits: u128,
    /// How many of the top bits are known, `rseq` included: `7n + 7`.
    /// The bits below are unknown, not "missing".
    pub count: u32,
    /// The single/multi flag: true for a multi datagram.
    pub multi: bool,
    /// The number of tail bytes, `n`.
    pub tail: usize,
}

/// `b(k)` of a window whose bit 127 is `rseq`.
fn bit(bits: u128, k: usize) -> u8 {
    if k > 127 {
        0
    } else {
        (bits >> (127 - k) & 1) as u8
    }
}

/// Seven ack bits `b(first) .. b(first + 6)`, the first in bit 6.
fn group(bits: u128, first: usize) -> u8 {
    (0..7).fold(0, |byte, i| byte | bit(bits, first + i) << (6 - i))
}

/// Encode a window (bit 127 = the newest received seq) with `n` tail bytes.
/// Returns `ack0` and the tail in wire order (`t_n` first, `t_1` last).
pub fn encode(bits: u128, n: usize, multi: bool) -> (u8, Vec<u8>) {
    assert!(n <= MAX_TAIL, "at most {MAX_TAIL} tail ack bytes");
    let flag = u8::from(multi);
    if n == 0 {
        return (group(bits, 1) & 0x7E | flag, Vec::new());
    }
    let ack0 = 0x80 | group(bits, 1);
    let mut tail = Vec::with_capacity(n);
    tail.push(group(bits, 7 * n + 1) & 0x7E | flag);
    for i in (1..n).rev() {
        tail.push(0x80 | group(bits, 7 * i + 1));
    }
    (ack0, tail)
}

/// The fewest tail bytes that carry `b(1) ..= b(depth)`.
pub fn tail_for_depth(depth: usize) -> usize {
    depth.saturating_sub(6).div_ceil(7).min(MAX_TAIL)
}

/// Decode the ack field of a whole datagram. `None` when the datagram is
/// shorter than its header, has more than 18 tail bytes, or the tail runs
/// into the header (the client rejects the last at 0x3FA1B7).
pub fn decode(datagram: &[u8]) -> Option<Acks> {
    if datagram.len() < 3 {
        return None;
    }
    let mut bits = 1u128 << 127;
    let mut known = 1usize;
    let mut byte = datagram[2];
    let mut tail = 0;
    while byte & 0x80 != 0 {
        for i in 0..7 {
            if byte >> (6 - i) & 1 != 0 && known + i <= 127 {
                bits |= 1 << (127 - (known + i));
            }
        }
        known += 7;
        tail += 1;
        if tail > MAX_TAIL_READ || datagram.len() < 3 + tail {
            return None;
        }
        byte = datagram[datagram.len() - tail];
    }
    for i in 0..6 {
        if byte >> (6 - i) & 1 != 0 && known + i <= 127 {
            bits |= 1 << (127 - (known + i));
        }
    }
    Some(Acks {
        bits,
        count: (known + 6).min(127) as u32,
        multi: byte & 1 != 0,
        tail,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A window from the list of `k` with `b(k) = 1`.
    fn window(back: &[usize]) -> u128 {
        back.iter().fold(1 << 127, |bits, k| bits | 1 << (127 - k))
    }

    fn datagram(bits: u128, n: usize, multi: bool, body: &[u8]) -> Vec<u8> {
        let (ack0, tail) = encode(bits, n, multi);
        let mut datagram = vec![9, 5, ack0];
        datagram.extend(body);
        datagram.extend(tail);
        datagram
    }

    #[test]
    fn the_keepalive_of_the_note_has_ack0_0x50() {
        // transport.md §9 (a): received 5, 4 and 2, so b(1) and b(3).
        assert_eq!(encode(window(&[1, 3]), 0, false), (0x50, vec![]));
        assert_eq!(
            datagram(window(&[1, 3]), 0, false, &[0xFC]),
            [9, 5, 0x50, 0xFC]
        );
    }

    #[test]
    fn the_flag_sits_in_the_terminating_byte() {
        assert_eq!(encode(1 << 127, 0, true), (0x01, vec![]));
        assert_eq!(encode(1 << 127, 1, true), (0x80, vec![0x01]));
        assert_eq!(encode(1 << 127, 2, false), (0x80, vec![0x00, 0x80]));
        // b(1) in ack0 bit 6; b(8) in t_1 bit 6; b(14) in t_1 bit 0;
        // b(15) in t_2 bit 6; b(20) in t_2 bit 1; b(21) is not carried.
        let bits = window(&[1, 8, 14, 15, 20, 21]);
        assert_eq!(encode(bits, 2, true), (0xC0, vec![0x43, 0xC1]));
        // With one tail byte t_1 terminates: b(8)..b(13) in bits 6..1.
        assert_eq!(encode(bits, 1, false), (0xC0, vec![0x40]));
    }

    #[test]
    fn eighteen_tail_bytes_are_read_and_nineteen_are_not() {
        // Every byte a continuation with all its bits: b(1)..b(126), then
        // the terminating byte's b(127) (its lower bits are past the
        // window).
        let mut datagram = vec![9, 5, 0xFF, 0xFC];
        datagram.push(0x7E); // t_18, terminating
        datagram.extend([0xFF; 17]); // t_17 .. t_1
        let acks = decode(&datagram).unwrap();
        assert_eq!(acks.tail, 18);
        assert_eq!(acks.count, 127);
        assert_eq!(acks.bits, u128::MAX);
        assert!(!acks.multi);
        // The flag is still the terminating byte's bit 0.
        datagram[4] = 0x01;
        let acks = decode(&datagram).unwrap();
        assert!(acks.multi);
        assert_eq!(acks.bits, u128::MAX << 1);
        let mut longer = vec![9, 5, 0xFF, 0xFC, 0x00];
        longer.extend([0xFF; 18]);
        assert_eq!(decode(&longer), None);
    }

    #[test]
    fn every_tail_length_round_trips() {
        // A fixed irregular pattern over all 127 back bits.
        let back: Vec<usize> = (1..=127).filter(|k| k * k % 7 < 4 || k % 5 == 0).collect();
        let bits = window(&back);
        for n in 0..=MAX_TAIL {
            for multi in [false, true] {
                let acks = decode(&datagram(bits, n, multi, &[0xFC])).unwrap();
                let carried = 7 * n + 6;
                assert_eq!(acks.tail, n);
                assert_eq!(acks.multi, multi);
                assert_eq!(acks.count as usize, carried + 1);
                let expected: Vec<usize> = back.iter().copied().filter(|k| *k <= carried).collect();
                assert_eq!(acks.bits, window(&expected), "n = {n}");
            }
        }
    }

    #[test]
    fn the_depth_picks_the_tail_length() {
        assert_eq!(tail_for_depth(0), 0);
        assert_eq!(tail_for_depth(6), 0);
        assert_eq!(tail_for_depth(7), 1);
        assert_eq!(tail_for_depth(13), 1);
        assert_eq!(tail_for_depth(14), 2);
        assert_eq!(tail_for_depth(125), 17);
        assert_eq!(tail_for_depth(127), 17);
    }

    #[test]
    fn a_broken_tail_is_rejected() {
        assert_eq!(decode(&[0, 0]), None);
        // ack0 promises a tail byte that is not there.
        assert_eq!(decode(&[0, 0, 0x80]), None);
        // Tail bytes that never terminate.
        assert_eq!(decode(&[0x80; 40]), None);
        // The shortest valid ones.
        assert!(decode(&[0, 0, 0]).is_some());
        assert_eq!(decode(&[0, 0, 0x80, 0x01]).map(|acks| acks.tail), Some(1));
    }
}
