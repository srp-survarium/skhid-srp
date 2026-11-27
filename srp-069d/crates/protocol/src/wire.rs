//! Byte-level reader and writer shared by every message.
//!
//! Strings are the client's string reader 0x20011DE30 (PROVEN): `u8 len`, or `FF` + `u16 len`
//! for 255 bytes and more, then the bytes (no NUL). The client copies at most its buffer cap and skips the rest.

use crate::Error;

pub type Result<T> = std::result::Result<T, Error>;

/// Cursor over a payload. Every read is bounds checked (the client itself has no checks, we do).
pub struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Reader { buf, pos: 0 }
    }
    pub fn pos(&self) -> usize {
        self.pos
    }
    pub fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }
    pub fn is_empty(&self) -> bool {
        self.remaining() == 0
    }
    /// All bytes not read yet, consuming them.
    pub fn rest(&mut self) -> &'a [u8] {
        let s = &self.buf[self.pos..];
        self.pos = self.buf.len();
        s
    }
    /// Error when bytes are left over.
    pub fn finish(&self) -> Result<()> {
        if self.remaining() == 0 {
            Ok(())
        } else {
            Err(Error::Trailing(self.remaining()))
        }
    }
    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8]> {
        if self.remaining() < n {
            return Err(Error::Truncated { at: self.pos, need: n - self.remaining() });
        }
        let s = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }
    pub fn skip(&mut self, n: usize) -> Result<()> {
        self.bytes(n).map(|_| ())
    }
    pub fn u8(&mut self) -> Result<u8> {
        Ok(self.bytes(1)?[0])
    }
    pub fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.bytes(2)?.try_into().unwrap()))
    }
    pub fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.bytes(4)?.try_into().unwrap()))
    }
    pub fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.bytes(8)?.try_into().unwrap()))
    }
    pub fn i32(&mut self) -> Result<i32> {
        Ok(self.u32()? as i32)
    }
    pub fn f32(&mut self) -> Result<f32> {
        Ok(f32::from_le_bytes(self.bytes(4)?.try_into().unwrap()))
    }
    pub fn f64(&mut self) -> Result<f64> {
        Ok(f64::from_le_bytes(self.bytes(8)?.try_into().unwrap()))
    }
    pub fn array<const N: usize>(&mut self) -> Result<[u8; N]> {
        Ok(self.bytes(N)?.try_into().unwrap())
    }
    /// A `u8`-counted list needs no guard; this checks that `n` elements of at least `min_each` bytes can still
    /// follow, so a hostile count cannot allocate gigabytes.
    pub fn check_count(&self, n: usize, min_each: usize) -> Result<usize> {
        if n.saturating_mul(min_each) > self.remaining() {
            return Err(Error::Truncated { at: self.pos, need: n.saturating_mul(min_each).saturating_sub(self.remaining()) });
        }
        Ok(n)
    }
    /// Length-prefixed string bytes (`u8 len` / `FF u16 len`).
    pub fn str_bytes(&mut self) -> Result<&'a [u8]> {
        let n = match self.u8()? {
            0xFF => self.u16()? as usize,
            n => n as usize,
        };
        self.bytes(n)
    }
    /// Length-prefixed string; invalid UTF-8 is replaced (the client treats the bytes as opaque).
    pub fn str(&mut self) -> Result<String> {
        Ok(String::from_utf8_lossy(self.str_bytes()?).into_owned())
    }
}

/// Output buffer. The first encoding error (a count or string that cannot be represented) is kept and
/// returned by [`Writer::finish`]; later writes still succeed so `encode` bodies stay simple.
#[derive(Default)]
pub struct Writer {
    buf: Vec<u8>,
    err: Option<Error>,
}

impl Writer {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn len(&self) -> usize {
        self.buf.len()
    }
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }
    pub fn fail(&mut self, e: Error) {
        if self.err.is_none() {
            self.err = Some(e);
        }
    }
    pub fn finish(self) -> Result<Vec<u8>> {
        match self.err {
            Some(e) => Err(e),
            None => Ok(self.buf),
        }
    }
    pub fn u8(&mut self, v: u8) -> &mut Self {
        self.buf.push(v);
        self
    }
    pub fn u16(&mut self, v: u16) -> &mut Self {
        self.buf.extend_from_slice(&v.to_le_bytes());
        self
    }
    pub fn u32(&mut self, v: u32) -> &mut Self {
        self.buf.extend_from_slice(&v.to_le_bytes());
        self
    }
    pub fn u64(&mut self, v: u64) -> &mut Self {
        self.buf.extend_from_slice(&v.to_le_bytes());
        self
    }
    pub fn i32(&mut self, v: i32) -> &mut Self {
        self.u32(v as u32)
    }
    pub fn f32(&mut self, v: f32) -> &mut Self {
        self.buf.extend_from_slice(&v.to_le_bytes());
        self
    }
    pub fn f64(&mut self, v: f64) -> &mut Self {
        self.buf.extend_from_slice(&v.to_le_bytes());
        self
    }
    pub fn bytes(&mut self, b: &[u8]) -> &mut Self {
        self.buf.extend_from_slice(b);
        self
    }
    pub fn zeros(&mut self, n: usize) -> &mut Self {
        self.buf.resize(self.buf.len() + n, 0);
        self
    }
    /// A count that the client stores in one byte.
    pub fn count_u8(&mut self, n: usize, what: &'static str) -> &mut Self {
        if n > 0xFF {
            self.fail(Error::Limit(what));
        }
        self.u8(n as u8)
    }
    pub fn count_u16(&mut self, n: usize, what: &'static str) -> &mut Self {
        if n > 0xFFFF {
            self.fail(Error::Limit(what));
        }
        self.u16(n as u16)
    }
    pub fn count_u32(&mut self, n: usize) -> &mut Self {
        self.u32(n as u32)
    }
    /// Length-prefixed raw bytes, no cap (the client's cap is checked by the caller, see [`Writer::str_max`]).
    pub fn str_raw(&mut self, b: &[u8]) -> &mut Self {
        if b.len() < 255 {
            self.u8(b.len() as u8);
        } else {
            self.u8(0xFF);
            self.count_u16(b.len(), "string length");
        }
        self.bytes(b)
    }
    pub fn str(&mut self, s: &str) -> &mut Self {
        self.str_raw(s.as_bytes())
    }
    /// String cut to `max` bytes (the cut may fall inside a UTF-8 sequence;
    /// the client does not care). The client's own buffers hold `max`; longer text is skipped by it anyway.
    pub fn str_max(&mut self, s: &str, max: usize) -> &mut Self {
        let b = s.as_bytes();
        self.str_raw(&b[..b.len().min(max)])
    }
}

/// A message body that can be written.
pub trait Encode {
    fn encode(&self, w: &mut Writer);
    fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut w = Writer::new();
        self.encode(&mut w);
        w.finish()
    }
}

/// A message body that can be read.
pub trait Decode: Sized {
    fn decode(r: &mut Reader) -> Result<Self>;
    /// Strict: the whole slice must be consumed.
    fn from_bytes(b: &[u8]) -> Result<Self> {
        let mut r = Reader::new(b);
        let v = Self::decode(&mut r)?;
        r.finish()?;
        Ok(v)
    }
}
