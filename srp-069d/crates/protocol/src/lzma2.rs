//! Raw LZMA2 as the client's packed readers take it (no .xz / .lzma header).
//!
//! PROVEN: the client decodes with 7-zip `Lzma2Decode`, dictionary property byte fixed 0
//! (4 KiB); a stream of stored chunks (`01 hh ll data`, ..., `00` end mark) is valid and needs no compressor.
//! Compressed streams (for example python `lzma.FORMAT_RAW` with `FILTER_LZMA2`, `dict_size` 4096) decode here too.
//! Encoding writes stored chunks (larger on the wire than compressed output, accepted by the same decoder).
//! An empty stream is the single byte `00`; a zero-length input is a decoder failure in the client (int 3).

use crate::{Error, Reader, Result, Writer};

/// Largest payload of one stored chunk.
const CHUNK: usize = 0x10000;

/// Raw LZMA2 stream made of stored (uncompressed) chunks.
pub fn compress(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(raw.len() + raw.len() / CHUNK * 3 + 4);
    for (i, c) in raw.chunks(CHUNK).enumerate() {
        out.push(if i == 0 { 0x01 } else { 0x02 }); // 1 = stored + dictionary reset, 2 = stored, no reset
        out.extend_from_slice(&((c.len() - 1) as u16).to_be_bytes());
        out.extend_from_slice(c);
    }
    out.push(0x00);
    out
}

/// Decode a raw LZMA2 stream.
pub fn decompress(blob: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut input = std::io::Cursor::new(blob);
    lzma_rs::lzma2_decompress(&mut input, &mut out).map_err(|e| Error::Lzma(format!("{e:?}")))?;
    Ok(out)
}

/// `u32 size` + raw LZMA2: the form of 0x89 and of the 0x5B reply.
pub fn write_packed(w: &mut Writer, raw: &[u8]) {
    let blob = compress(raw);
    w.u32(blob.len() as u32).bytes(&blob);
}

/// Read `u32 size` + raw LZMA2 and decode it.
pub fn read_packed(r: &mut Reader) -> Result<Vec<u8>> {
    let n = r.u32()? as usize;
    decompress(r.bytes(n)?)
}
