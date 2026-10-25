//! Parse, don't validate: bytes from the network (a packet the server
//! sends) or from the game's memory are read once, at the boundary, into a
//! typed value; the code after it takes only that value.
//!
//! ```
//! use patch_core::{Parse, Reader};
//! struct Proposal { seed: u32, place: u8 }
//! impl<'a> Parse<'a> for Proposal {
//!     fn parse(r: &mut Reader<'a>) -> Option<Proposal> {
//!         r.expect(b"LJ")?;
//!         Some(Proposal { seed: r.u32()?, place: r.u8()? })
//!     }
//! }
//! assert!(Proposal::parse_all(b"LJ\x01\x00\x00\x00\x05").is_some());
//! assert!(Proposal::parse_all(b"LJ\x01\x00").is_none());
//! ```

/// Little-endian bytes, read from the front; a read past the end is none.
#[derive(Clone, Debug)]
pub struct Reader<'a> {
    bytes: &'a [u8],
}

impl<'a> Reader<'a> {
    pub fn new(bytes: &'a [u8]) -> Reader<'a> {
        Reader { bytes }
    }

    /// The bytes of the game's memory at `at`.
    ///
    /// # Safety
    /// `len` bytes from `at` are readable and not written while it reads.
    pub unsafe fn of_memory(at: *const u8, len: usize) -> Reader<'a> {
        Reader { bytes: unsafe { core::slice::from_raw_parts(at, len) } }
    }

    pub fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let (taken, rest) = self.bytes.split_at_checked(n)?;
        self.bytes = rest;
        Some(taken)
    }

    pub fn array<const N: usize>(&mut self) -> Option<[u8; N]> {
        let (taken, rest) = self.bytes.split_first_chunk::<N>()?;
        self.bytes = rest;
        Some(*taken)
    }

    pub fn u8(&mut self) -> Option<u8> {
        self.array::<1>().map(|[b]| b)
    }

    pub fn u16(&mut self) -> Option<u16> {
        self.array().map(u16::from_le_bytes)
    }

    pub fn u32(&mut self) -> Option<u32> {
        self.array().map(u32::from_le_bytes)
    }

    pub fn f32(&mut self) -> Option<f32> {
        self.array().map(f32::from_le_bytes)
    }

    /// These bytes next, or none.
    pub fn expect(&mut self, bytes: &[u8]) -> Option<()> {
        (self.take(bytes.len())? == bytes).then_some(())
    }

    pub fn remaining(&self) -> &'a [u8] {
        self.bytes
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
}

/// A value read from bytes.
pub trait Parse<'a>: Sized {
    /// The value at the reader's front, the reader past it; none for bytes
    /// that are not one.
    fn parse(r: &mut Reader<'a>) -> Option<Self>;

    /// The value that is all of `bytes`.
    fn parse_all(bytes: &'a [u8]) -> Option<Self> {
        let mut r = Reader::new(bytes);
        let value = Self::parse(&mut r)?;
        r.is_empty().then_some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_little_endian_and_stops_at_the_end() {
        let mut r = Reader::new(&[1, 2, 3, 4, 5, 6, 7]);
        assert_eq!(r.u16(), Some(0x0201));
        assert_eq!(r.u32(), Some(0x0605_0403));
        assert_eq!(r.u16(), None);
        assert_eq!(r.u8(), Some(7));
        assert!(r.is_empty());
    }
}
