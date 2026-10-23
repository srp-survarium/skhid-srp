//! A raw LZMA2 stream of stored (uncompressed) chunks, for the payloads the
//! 0.26g client unpacks with LZMA2: the match's static info 0xE4 and world
//! snapshot 0xE8, and the lobby's prices 0x8A and shop 0x95
//! (`docs/notes/match.md` §7, `static_info.md` §1, `lobby.md`).
//!
//! The wire has a bare LZMA2 chunk stream: no property byte, no .xz or .7z
//! container (PROVEN, the notes). LZMA2 lets a chunk be stored as it is:
//!
//! ```text
//! 01 <u16 BE size - 1> <size bytes>   the first chunk: stored, dictionary reset
//! 02 <u16 BE size - 1> <size bytes>   any later chunk: stored, no reset
//! 00                                  the end of the stream
//! ```
//!
//! with `size` 1..=65536. So the server needs no LZMA encoder for those, at
//! the cost of 3 bytes for each 64 KiB and 1 for the end. The tests check
//! the bytes against Python's `lzma` (liblzma) raw LZMA2 decoder.
//!
//! The world snapshot is the one payload that pays for a real encoder
//! ([`packed`]): it goes to every client at every start and resync, as up
//! to 31 datagrams when stored, and a thin line does not carry that burst
//! (live, 2026-10-13: a player who never got one in two matches). Saved
//! worlds of live matches shrink 2.6 times.
//!
//! **Chunks larger than the client's dictionary.** The client's decoder
//! object has a 32 KiB dictionary (property byte 6), and a chunk here holds
//! up to 64 KiB. That is no problem [LIKELY]:
//! - The client calls the LZMA SDK's one-call `Lzma2Decode` (0x3EE810),
//!   which sets the decoder's dictionary buffer to the **output buffer
//!   itself** (`dic = dest`, `dicBufSize = destLen`: the stores at
//!   0x3EE84B and 0x3EE84F; PROVEN). The property byte only sizes the
//!   `dicSize` used to check match distances.
//! - A stored chunk is copied straight to `dic + dicPos`
//!   (`LzmaDec_UpdateWithUncompressed`); it has no match distances to
//!   check, and nothing wraps because the buffer is the whole output.
//!   This part is the stock SDK's source, not traced in the exe: LIKELY.
//! - liblzma decodes the same streams with `dict_size` 0x8000 (the tests),
//!   though it keeps a real window.
//!
//! What does matter is the client's **output buffer**: the decode's result
//! is not checked, so never send more than the reader's buffer holds
//! (0x578A0 bytes for the static info, 0x10000 for a compressed snapshot
//! with patch 60, 0x8000 before it, `n × 8` for the prices). A snapshot of
//! up to 0xFF04 bytes is one stream of chunks with the 32 KiB dictionary:
//! its matches reach no further back than 0x8000 (the encoder's
//! `dict_size`), whatever the length of the output.

use std::io::{Read, Write};

use lzma_rust2::{Lzma2Options, Lzma2Reader, Lzma2Writer, LzmaOptions};

/// The most bytes one chunk holds.
pub const CHUNK: usize = 0x1_0000;

/// The client's decoder checks match distances against a dictionary of
/// this size (property byte 6, `static_info.md` section 1).
pub const DICTIONARY: u32 = 0x8000;

/// The encoder's effort (xz's preset). The match server packs a world in
/// the thread that relays the inputs: on 120 saved live worlds preset 3
/// took a third of the time of the default 6 for 2 % more bytes (2.62
/// against 2.67 times smaller; 0.6 ms a world on a desktop).
const PRESET: u32 = 3;

/// A stored chunk with a dictionary reset: the first of a stream.
const STORED_RESET: u8 = 0x01;
/// A stored chunk without a reset.
const STORED: u8 = 0x02;
/// The end of the stream.
const END: u8 = 0x00;

/// The size of [`stored`]'s result for `len` bytes of data.
pub fn stored_len(len: usize) -> usize {
    len + 3 * len.div_ceil(CHUNK) + 1
}

/// `data` as a raw LZMA2 stream of stored chunks. Empty data is the end
/// marker alone.
pub fn stored(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(stored_len(data.len()));
    for (index, chunk) in data.chunks(CHUNK).enumerate() {
        out.push(if index == 0 { STORED_RESET } else { STORED });
        out.extend_from_slice(&((chunk.len() - 1) as u16).to_be_bytes());
        out.extend_from_slice(chunk);
    }
    out.push(END);
    out
}

/// Reads a stream [`stored`] wrote back into its data: `None` if it is not
/// one (a compressed chunk, a bad first chunk, a cut stream, or bytes after
/// the end). For tests and for checking a writer's output; the client never
/// sends LZMA2 that srp must read this way.
pub fn unstore(stream: &[u8]) -> Option<Vec<u8>> {
    let mut data = Vec::new();
    let mut rest = stream;
    let mut first = true;
    loop {
        let (&control, tail) = rest.split_first()?;
        match control {
            END => return tail.is_empty().then_some(data),
            STORED_RESET => {}
            STORED if !first => {}
            _ => return None,
        }
        let size = usize::from(u16::from_be_bytes([*tail.first()?, *tail.get(1)?])) + 1;
        data.extend_from_slice(tail.get(2..2 + size)?);
        rest = &tail[2 + size..];
        first = false;
    }
}

/// `data` as a raw LZMA2 stream of compressed chunks, for a reader that
/// decodes with the client's dictionary ([`DICTIONARY`]): the same bare
/// chunk stream as [`stored`], ended by the same marker, with the LZMA
/// SDK's default `lc`, `lp` and `pb` (3, 0, 2: the client's `Lzma2Decode`
/// allocates for `lc + lp` up to 4). The first chunk resets the dictionary,
/// the state and the properties, as the decoder demands of a first chunk.
///
/// srp sent no real client a compressed chunk before 2026-10-13: the
/// client's code is the stock SDK's one-call decoder (0x3EE810), which
/// reads such streams, but that it does so in the game is LIKELY until it
/// is seen live.
pub fn packed(data: &[u8]) -> Vec<u8> {
    let options = Lzma2Options {
        lzma_options: LzmaOptions {
            dict_size: DICTIONARY,
            ..LzmaOptions::with_preset(PRESET)
        },
        chunk_size: None,
    };
    let mut writer = Lzma2Writer::new(Vec::with_capacity(data.len() / 2 + 16), options);
    writer
        .write_all(data)
        .expect("writing to memory does not fail");
    writer.finish().expect("writing to memory does not fail")
}

/// Reads any raw LZMA2 stream back into its data, as the client does:
/// stored chunks, compressed ones, or both. `None` if it is no such stream,
/// or if the data is over `max` bytes (the client decodes into a buffer of
/// a fixed size and does not check the result).
pub fn unpack(stream: &[u8], max: usize) -> Option<Vec<u8>> {
    let mut data = Vec::new();
    Lzma2Reader::new(stream, DICTIONARY, None)
        .take(max as u64 + 1)
        .read_to_end(&mut data)
        .ok()?;
    (data.len() <= max).then_some(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bytes that do not repeat with the chunk size.
    fn pattern(len: usize) -> Vec<u8> {
        (0..len).map(|i| (i * 31 + i / 251) as u8).collect()
    }

    #[test]
    fn empty_data_is_the_end_marker() {
        assert_eq!(stored(&[]), [0x00]);
        assert_eq!(stored_len(0), 1);
    }

    #[test]
    fn one_byte() {
        assert_eq!(stored(&[0xAB]), [0x01, 0x00, 0x00, 0xAB, 0x00]);
    }

    #[test]
    fn a_few_bytes() {
        assert_eq!(stored(b"srp"), [0x01, 0x00, 0x02, b's', b'r', b'p', 0x00]);
        // 256 bytes: the size is big-endian.
        let data = pattern(256);
        let stream = stored(&data);
        assert_eq!(stream[..3], [0x01, 0x00, 0xFF]);
        assert_eq!(stream[3..259], data[..]);
        assert_eq!(stream[259..], [0x00]);
        let stream = stored(&pattern(257));
        assert_eq!(stream[..3], [0x01, 0x01, 0x00]);
    }

    #[test]
    fn exactly_one_full_chunk() {
        let data = pattern(65536);
        let stream = stored(&data);
        assert_eq!(stream.len(), 65536 + 4);
        assert_eq!(stream[..3], [0x01, 0xFF, 0xFF]);
        assert_eq!(stream[3..65539], data[..]);
        assert_eq!(stream[65539..], [0x00]);
    }

    #[test]
    fn one_byte_over_a_chunk() {
        let data = pattern(65537);
        let stream = stored(&data);
        assert_eq!(stream.len(), 65537 + 3 + 3 + 1);
        assert_eq!(stream[..3], [0x01, 0xFF, 0xFF]);
        assert_eq!(stream[3..65539], data[..65536]);
        // The second chunk does not reset the dictionary.
        assert_eq!(stream[65539..65542], [0x02, 0x00, 0x00]);
        assert_eq!(stream[65542], data[65536]);
        assert_eq!(stream[65543..], [0x00]);
    }

    #[test]
    fn two_hundred_thousand_bytes() {
        let data = pattern(200_000);
        let stream = stored(&data);
        // Three full chunks and 3392 bytes.
        assert_eq!(stream.len(), 200_000 + 4 * 3 + 1);
        assert_eq!(stream.len(), stored_len(200_000));
        let header = |at: usize| [stream[at], stream[at + 1], stream[at + 2]];
        assert_eq!(header(0), [0x01, 0xFF, 0xFF]);
        assert_eq!(header(65539), [0x02, 0xFF, 0xFF]);
        assert_eq!(header(131078), [0x02, 0xFF, 0xFF]);
        assert_eq!(header(196617), [0x02, 0x0D, 0x3F]); // 3392 - 1
        assert_eq!(stream[196620..200_012], data[196608..]);
        assert_eq!(stream[200_012..], [0x00]);
    }

    #[test]
    fn unstore_reads_back_what_stored_wrote() {
        for len in [0, 1, 2, 255, 256, 65535, 65536, 65537, 131072, 200_000] {
            let data = pattern(len);
            let stream = stored(&data);
            assert_eq!(stream.len(), stored_len(len), "{len}");
            assert_eq!(unstore(&stream).as_deref(), Some(&data[..]), "{len}");
        }
        // Not streams of stored chunks.
        assert_eq!(unstore(&[]), None);
        assert_eq!(unstore(&[0x00, 0x00]), None); // bytes after the end
        assert_eq!(unstore(&[0x02, 0x00, 0x00, 0xAB, 0x00]), None); // no reset first
        assert_eq!(unstore(&[0x01, 0x00, 0x00, 0xAB]), None); // no end
        assert_eq!(unstore(&[0x01, 0x00, 0x01, 0xAB, 0x00]), None); // cut
        assert_eq!(unstore(&[0xE0, 0x00, 0x00, 0x00, 0x00, 0x00]), None); // LZMA chunk
    }

    /// The streams decode with a real LZMA2 decoder: Python's `lzma`
    /// (liblzma), raw LZMA2 with the client's 32 KiB dictionary. Skipped,
    /// with a note, where there is no `python`; by hand:
    /// `python -I scripts/gen-data-026g/check_lzma2.py <dir of *.lzma2 + *.bin>`.
    #[test]
    fn a_real_decoder_reads_the_streams() {
        let streams: Vec<_> = [0, 1, 65536, 65537, 200_000, 0x578A0]
            .into_iter()
            .map(|len| {
                let data = pattern(len);
                let stream = stored(&data);
                (data, stream)
            })
            .collect();
        real_decoder_reads("stored", &streams);
    }

    /// Bytes as a world has them: runs of zeros, repeated blocks, and some
    /// that do not compress.
    fn world_like(len: usize) -> Vec<u8> {
        (0..len)
            .map(|i| match i % 1024 {
                0..300 => 0,
                300..700 => (i % 37) as u8,
                _ => (i * 31 + i / 251) as u8 ^ (i >> 3) as u8,
            })
            .collect()
    }

    #[test]
    fn packed_is_smaller_and_comes_back() {
        for len in [0, 1, 2, 500, 9_000, 14_000, 0x8000, 0xFF04] {
            let data = world_like(len);
            let stream = packed(&data);
            assert_eq!(unpack(&stream, 0x1_0000).as_deref(), Some(&data[..]), "{len}");
            assert_eq!(stream.last(), Some(&0x00), "{len}: the end marker");
            if len >= 500 {
                assert!(stream.len() < len, "{len}: {} bytes", stream.len());
                // A first chunk the decoder takes: compressed, with a reset
                // of the dictionary, the state and the properties.
                assert!(stream[0] >= 0xE0, "{len}: control {:#04x}", stream[0]);
            }
        }
        assert_eq!(packed(&[]), [0x00]);
        // Bytes that do not compress come back all the same.
        let noise: Vec<u8> = (0..4096u32)
            .map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8)
            .collect();
        assert_eq!(unpack(&packed(&noise), 0x8000), Some(noise));
    }

    #[test]
    fn unpack_reads_stored_streams_and_minds_the_buffer() {
        let data = pattern(0x8000);
        assert_eq!(unpack(&stored(&data), 0x8000), Some(data.clone()));
        assert_eq!(unpack(&packed(&data), 0x8000), Some(data));
        // One byte over the reader's buffer.
        let over = pattern(0x8001);
        assert_eq!(unpack(&stored(&over), 0x8000), None);
        assert_eq!(unpack(&packed(&over), 0x8000), None);
        // No stream at all, and one cut short.
        assert_eq!(unpack(&[0x03], 0x8000), None);
        let stream = packed(&world_like(9_000));
        assert_eq!(unpack(&stream[..stream.len() / 2], 0x8000), None);
    }

    /// The compressed streams too, with the client's 32 KiB dictionary.
    #[test]
    fn a_real_decoder_reads_the_packed_streams() {
        let streams: Vec<_> = [1, 500, 9_000, 14_000, 0x8000, 0xFF04]
            .into_iter()
            .map(|len| {
                let data = world_like(len);
                let stream = packed(&data);
                (data, stream)
            })
            .collect();
        real_decoder_reads("packed", &streams);
    }

    fn real_decoder_reads(name: &str, streams: &[(Vec<u8>, Vec<u8>)]) {
        use std::process::Command;

        let dir = std::env::temp_dir().join(format!("srp-lzma2-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for (index, (data, stream)) in streams.iter().enumerate() {
            std::fs::write(dir.join(format!("{index}.bin")), data).unwrap();
            std::fs::write(dir.join(format!("{index}.lzma2")), stream).unwrap();
        }
        let script = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../scripts/gen-data-026g/check_lzma2.py"
        );
        let run = Command::new("python")
            .arg("-I")
            .arg(script)
            .arg(&dir)
            .output();
        let _ = std::fs::remove_dir_all(&dir);
        match run {
            Ok(output) => {
                let text = String::from_utf8_lossy(&output.stdout);
                assert!(
                    output.status.success()
                        && text.contains(&format!("{} streams decoded", streams.len())),
                    "{text}{}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
            Err(error) => eprintln!("python not run, LZMA2 decoder check skipped: {error}"),
        }
    }
}
