//! A bounds-checked cursor over little-endian bytes.

/// The bytes ended: at `at` (an offset of the message) `wanted` more were
/// needed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Short {
    pub at: usize,
    pub wanted: usize,
}

/// A cursor over a part of a message. Offsets ([`Reader::at`]) are the
/// message's: the part's own plus `base`, where the part starts.
#[derive(Clone, Copy, Debug)]
pub struct Reader<'a> {
    bytes: &'a [u8],
    base: usize,
    at: usize,
}

impl<'a> Reader<'a> {
    /// A cursor at the start of `bytes`, which start at `base` in the
    /// message.
    pub fn new(bytes: &'a [u8], base: usize) -> Self {
        Self { bytes, base, at: 0 }
    }

    /// Where the cursor is, in the message.
    pub fn at(&self) -> usize {
        self.base.saturating_add(self.at)
    }

    /// The bytes not read yet.
    pub fn rest(&self) -> &'a [u8] {
        self.bytes.get(self.at..).unwrap_or_default()
    }

    /// Whether every byte was read.
    pub fn is_done(&self) -> bool {
        self.rest().is_empty()
    }

    /// The next `len` bytes.
    pub fn take(&mut self, len: usize) -> Result<&'a [u8], Short> {
        let short = Short {
            at: self.at(),
            wanted: len,
        };
        let end = self.at.checked_add(len).ok_or(short)?;
        let bytes = self.bytes.get(self.at..end).ok_or(short)?;
        self.at = end;
        Ok(bytes)
    }

    /// Steps over `len` bytes.
    pub fn skip(&mut self, len: usize) -> Result<(), Short> {
        self.take(len).map(|_| ())
    }

    pub fn array<const N: usize>(&mut self) -> Result<[u8; N], Short> {
        let at = self.at();
        self.take(N)?
            .try_into()
            .map_err(|_| Short { at, wanted: N })
    }

    pub fn u8(&mut self) -> Result<u8, Short> {
        Ok(u8::from_le_bytes(self.array()?))
    }

    pub fn u16(&mut self) -> Result<u16, Short> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    pub fn u32(&mut self) -> Result<u32, Short> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    /// An f32's bits (see [`finite`](crate::finite)).
    pub fn f32_bits(&mut self) -> Result<u32, Short> {
        self.u32()
    }

    /// A bool as the game's writer puts it, 0x00 or 0xFF (0x80FA0).
    /// `None`: another byte.
    pub fn bool(&mut self) -> Result<Option<bool>, Short> {
        Ok(match self.u8()? {
            0x00 => Some(false),
            0xFF => Some(true),
            _ => None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_read_past_the_end_says_where() {
        let mut reader = Reader::new(&[1, 2, 3, 4, 5], 0x10);
        assert_eq!(reader.u32(), Ok(0x0403_0201));
        assert_eq!(reader.at(), 0x14);
        assert_eq!(
            reader.u16(),
            Err(Short {
                at: 0x14,
                wanted: 2
            })
        );
        assert_eq!(
            reader.take(usize::MAX),
            Err(Short {
                at: 0x14,
                wanted: usize::MAX
            })
        );
        assert_eq!(reader.u8(), Ok(5));
        assert!(reader.is_done());
    }

    #[test]
    fn a_bool_is_0x00_or_0xff() {
        let mut reader = Reader::new(&[0x00, 0xFF, 0x01], 0);
        assert_eq!(reader.bool(), Ok(Some(false)));
        assert_eq!(reader.bool(), Ok(Some(true)));
        assert_eq!(reader.bool(), Ok(None));
    }
}
