//! The match link's transport-level bounds for the two bodies a client
//! decompresses from bytes that began in another client: the world
//! snapshot 0xE8 and the static info 0xE4 (`docs/notes/relay_trust.md`
//! section 3, `world_validation.md` section 11).
//!
//! The client unpacks both with the LZMA SDK's one-call LZMA2 decoder
//! (`Lzma2Decode` 0x3EE810, reached through the vtable wrapper 0x357EB0):
//! - 0xE8: the reader 0x27BFF0 reserves a 0x8000 buffer and passes
//!   `*destLen` = 0x8000 to the decoder (0x27C059, 0x27C081; PROVEN).
//! - 0xE4: the reader 0x27B880 reserves a 0x578A0 buffer and passes
//!   `*destLen` = 0x578A0 (0x27B898, 0x27B8BB; PROVEN).
//!
//! The SDK honours `*destLen` and stops at it, so the decode never writes
//! past the buffer: the output cap is a hard native bound (PROVEN). What
//! the readers then skip is the decode's return value (`LZMA_STATUS_*` is
//! not checked, 0x27C0A1 / 0x27B8E9), so a stream that decodes short or
//! errors leaves a part-filled buffer, which the body readers below
//! (`crate::world`, `crate::messages::options`) then fault.
//!
//! On srp's match link no client ever chooses these LZMA2 bytes: the
//! server re-makes both streams from its own bounded data (the raw upload
//! body, at most 0x8000 by `upload.rs`; the options buffer, at most
//! 0x578A0 by `static_info.rs::options_bytes`), compressed with
//! `survarium::v026g::lzma2`. [`lzma2_bound`] is the shared, allocation-free
//! guard both ends still run: the server declines to send a packed stream
//! that would not decode within the cap (`match_msgs.rs`), and the exe
//! payload refuses one at the dispatch before the native decoder
//! (`crate::messages::check`), a loud refusal in place of a part-filled
//! buffer.

use crate::Reason;
use crate::reader::Reader;

/// The 0xE8 state buffer (`u32 time` then the world body): the client
/// decodes into 0x8000 bytes (`client_model::COMPRESSED_SNAPSHOT_BUFFER`).
pub const SNAPSHOT_STATE_CAP: usize = 0x8000;

/// The 0xE4 options buffer: the client decodes into 0x578A0 bytes
/// (`messages::OPTIONS_BUFFER`, `static_info.md` section 1).
pub const OPTIONS_CAP: usize = 0x578A0;

/// The declared output of a raw LZMA2 chunk stream, read from the chunk
/// headers alone with no decode: each chunk states its own uncompressed
/// size (the xz LZMA2 format), so the sum is exactly what the native
/// decoder would produce. `Ok(len)` is that sum; it never runs past `cap`.
///
/// The walk also proves the stream is well formed and whole, so a cut or
/// junk stream is refused rather than fed to a decoder that ignores its
/// own result. `Err((at, reason))`: `at` is the offset in the stream.
///
/// A chunk is one of (LZMA2, no .xz or .7z container; `lzma2.rs`):
/// - `00`                           the end of the stream;
/// - `01 <u16 BE size-1> data`      a stored chunk, the first resetting
///   `02 <u16 BE size-1> data`      the dictionary (`01`);
/// - `1RRuuuuu <u16 BE unpacked-1> <u16 BE packed-1> [prop] data`
///   a compressed chunk: `uuuuu` the top five bits of `unpacked-1`, `RR`
///   the reset mode (a `prop` byte when `RR >= 2`, a dictionary reset when
///   `RR == 3`; the first chunk must reset the dictionary).
pub fn lzma2_bound(stream: &[u8], cap: usize) -> Result<usize, (usize, Reason)> {
    let mut reader = Reader::new(stream, 0);
    let mut total: usize = 0;
    let mut first = true;
    let cut = |short: crate::reader::Short| (short.at, Reason::Lzma2Truncated);
    loop {
        let at = reader.at();
        let control = reader.u8().map_err(cut)?;
        if control == 0 {
            // The end marker: nothing may follow it.
            if reader.is_done() {
                return Ok(total);
            }
            return Err((reader.at(), Reason::Lzma2Trailing));
        }
        let unpacked = if control == 1 || control == 2 {
            if first && control != 1 {
                return Err((at, Reason::Lzma2NoReset { control }));
            }
            let size = usize::from(u16::from_be_bytes(reader.array::<2>().map_err(cut)?));
            // `size - 1` is on the wire; the size is one more, 1..=0x10000.
            let size = size.saturating_add(1);
            reader.skip(size).map_err(cut)?;
            size
        } else if control >= 0x80 {
            // RR is bits 5..6: a dictionary reset needs both (RR == 3), a
            // prop byte comes with bit 6 (RR >= 2).
            if first && control & 0x60 != 0x60 {
                return Err((at, Reason::Lzma2NoReset { control }));
            }
            let [b1, b2, b3, b4] = reader.array::<4>().map_err(cut)?;
            let unpacked =
                usize::try_from(u32::from_be_bytes([0, control & 0x1F, b1, b2])).unwrap_or(0);
            let packed = usize::from(u16::from_be_bytes([b3, b4]));
            if control & 0x40 != 0 {
                reader.skip(1).map_err(cut)?;
            }
            reader.skip(packed.saturating_add(1)).map_err(cut)?;
            unpacked.saturating_add(1)
        } else {
            return Err((at, Reason::Lzma2Control { control }));
        };
        total = total.saturating_add(unpacked);
        if total > cap {
            return Err((at, Reason::Lzma2TooLarge { len: total, cap }));
        }
        first = false;
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use std::vec::Vec;

    /// A stored chunk stream of `data`, the first chunk resetting the
    /// dictionary (as `survarium::v026g::lzma2::stored` writes).
    fn stored(data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut first = true;
        for chunk in data.chunks(0x1_0000) {
            out.push(if first { 0x01 } else { 0x02 });
            out.extend_from_slice(&((chunk.len() - 1) as u16).to_be_bytes());
            out.extend_from_slice(chunk);
            first = false;
        }
        out.push(0x00);
        out
    }

    /// One compressed chunk of `unpacked` declared output and `packed`
    /// filler bytes, resetting the dictionary (prop byte present).
    fn compressed(unpacked: usize, packed: usize) -> Vec<u8> {
        let u = (unpacked - 1) as u32;
        let mut out = Vec::new();
        out.push(0xE0 | ((u >> 16) as u8 & 0x1F));
        out.extend_from_slice(&(u as u16).to_be_bytes());
        out.extend_from_slice(&((packed - 1) as u16).to_be_bytes());
        out.push(0x00); // the LZMA prop byte (lc/lp/pb)
        out.extend(std::iter::repeat_n(0xAB, packed));
        out.push(0x00);
        out
    }

    #[test]
    fn a_stored_stream_sums_its_chunk_sizes() {
        assert_eq!(lzma2_bound(&stored(&[]), OPTIONS_CAP), Ok(0));
        assert_eq!(lzma2_bound(&stored(&[0xAB]), OPTIONS_CAP), Ok(1));
        assert_eq!(lzma2_bound(&stored(&[0; 256]), OPTIONS_CAP), Ok(256));
        // Three full chunks and a tail: the sum, read from the headers.
        assert_eq!(
            lzma2_bound(&stored(&[0; 200_000]), OPTIONS_CAP),
            Ok(200_000)
        );
    }

    #[test]
    fn a_compressed_chunk_states_its_output() {
        assert_eq!(
            lzma2_bound(&compressed(0x8000, 100), SNAPSHOT_STATE_CAP),
            Ok(0x8000)
        );
        assert_eq!(lzma2_bound(&compressed(1, 1), SNAPSHOT_STATE_CAP), Ok(1));
    }

    #[test]
    fn an_over_cap_stream_is_refused() {
        // One byte over each buffer, stored and compressed.
        assert_eq!(
            lzma2_bound(&stored(&[0; 0x8001]), SNAPSHOT_STATE_CAP),
            Err((
                0,
                Reason::Lzma2TooLarge {
                    len: 0x8001,
                    cap: SNAPSHOT_STATE_CAP
                }
            ))
        );
        assert!(matches!(
            lzma2_bound(&compressed(0x578A1, 10), OPTIONS_CAP),
            Err((_, Reason::Lzma2TooLarge { .. }))
        ));
    }

    #[test]
    fn a_cut_stream_is_refused() {
        let good = stored(&[0; 300]);
        // Lose the end marker, and lose the chunk body (the header stays).
        assert_eq!(
            lzma2_bound(&good[..good.len() - 1], OPTIONS_CAP),
            Err((good.len() - 1, Reason::Lzma2Truncated))
        );
        assert_eq!(
            lzma2_bound(&good[..3], OPTIONS_CAP),
            Err((3, Reason::Lzma2Truncated))
        );
        // An empty stream has not even the end marker.
        assert_eq!(
            lzma2_bound(&[], OPTIONS_CAP),
            Err((0, Reason::Lzma2Truncated))
        );
    }

    #[test]
    fn junk_and_trailing_and_no_reset_are_refused() {
        // A control byte that is no chunk.
        assert_eq!(
            lzma2_bound(&[0x03, 0x00], OPTIONS_CAP),
            Err((0, Reason::Lzma2Control { control: 0x03 }))
        );
        // Bytes after the end marker.
        assert_eq!(
            lzma2_bound(&[0x00, 0x00], OPTIONS_CAP),
            Err((1, Reason::Lzma2Trailing))
        );
        // A first chunk that does not reset the dictionary.
        assert_eq!(
            lzma2_bound(&[0x02, 0x00, 0x00, 0xAB, 0x00], OPTIONS_CAP),
            Err((0, Reason::Lzma2NoReset { control: 0x02 }))
        );
        assert_eq!(
            lzma2_bound(
                &compressed(4, 4)
                    .iter()
                    .copied()
                    .map(|b| if b == 0xE0 { 0x80 } else { b })
                    .collect::<Vec<_>>(),
                OPTIONS_CAP
            ),
            Err((0, Reason::Lzma2NoReset { control: 0x80 }))
        );
    }
}
