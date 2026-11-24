//! Reading data out of a 64-bit Windows executable (PE32+) by virtual address: enough to read a table of string
//! pointers from `survarium.exe` (e.g. the client's modifier name table). No code is read or run.

use crate::{Error, Result};

#[derive(Debug, Clone, Copy)]
struct Section {
    va: u64,
    vsize: u64,
    raw: u64,
    raw_size: u64,
}

/// A parsed executable image (the file bytes and its section map).
pub struct Image<'a> {
    bytes: &'a [u8],
    base: u64,
    sections: Vec<Section>,
}

fn rd<const N: usize>(b: &[u8], o: usize) -> Result<[u8; N]> {
    b.get(o..o + N).and_then(|s| s.try_into().ok()).ok_or_else(|| Error::format("executable", format!("offset {o:#x} outside the file")))
}

impl<'a> Image<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Image<'a>> {
        let bad = |why: &str| Error::format("executable", why.to_string());
        if bytes.get(..2) != Some(b"MZ") {
            return Err(bad("no MZ header"));
        }
        let pe = u32::from_le_bytes(rd(bytes, 0x3C)?) as usize;
        if bytes.get(pe..pe + 4) != Some(b"PE\0\0") {
            return Err(bad("no PE header"));
        }
        let coff = pe + 4;
        let nsec = u16::from_le_bytes(rd(bytes, coff + 2)?) as usize;
        let opt_size = u16::from_le_bytes(rd(bytes, coff + 16)?) as usize;
        let opt = coff + 20;
        if u16::from_le_bytes(rd(bytes, opt)?) != 0x20B {
            return Err(bad("not a 64-bit image"));
        }
        let base = u64::from_le_bytes(rd(bytes, opt + 24)?);
        let mut sections = Vec::with_capacity(nsec);
        for i in 0..nsec {
            let s = opt + opt_size + 40 * i;
            sections.push(Section {
                vsize: u32::from_le_bytes(rd(bytes, s + 8)?) as u64,
                va: u32::from_le_bytes(rd(bytes, s + 12)?) as u64,
                raw_size: u32::from_le_bytes(rd(bytes, s + 16)?) as u64,
                raw: u32::from_le_bytes(rd(bytes, s + 20)?) as u64,
            });
        }
        Ok(Image { bytes, base, sections })
    }

    /// The file offset of virtual address `va`, if it lies in a section's file data.
    pub fn offset_of(&self, va: u64) -> Option<usize> {
        let rva = va.checked_sub(self.base)?;
        let s = self.sections.iter().find(|s| rva >= s.va && rva < s.va + s.vsize.max(s.raw_size))?;
        let within = rva - s.va;
        (within < s.raw_size).then(|| (s.raw + within) as usize)
    }

    pub fn u64_at(&self, va: u64) -> Result<u64> {
        let o = self.offset_of(va).ok_or_else(|| Error::format("executable", format!("address {va:#x} has no file data")))?;
        Ok(u64::from_le_bytes(rd(self.bytes, o)?))
    }

    /// The NUL-terminated ASCII string at `va`.
    pub fn cstr_at(&self, va: u64) -> Result<String> {
        let o = self.offset_of(va).ok_or_else(|| Error::format("executable", format!("address {va:#x} has no file data")))?;
        let s = &self.bytes[o..];
        let n = s.iter().position(|&c| c == 0).ok_or_else(|| Error::format("executable", format!("string at {va:#x} not terminated")))?;
        if !s[..n].is_ascii() {
            return Err(Error::format("executable", format!("string at {va:#x} is not ASCII")));
        }
        Ok(String::from_utf8_lossy(&s[..n]).into_owned())
    }

    /// `count` strings through the table of 8-byte pointers at `va`.
    pub fn string_table(&self, va: u64, count: usize) -> Result<Vec<String>> {
        (0..count).map(|i| self.u64_at(va + 8 * i as u64).and_then(|p| self.cstr_at(p))).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal image: one section at RVA 0x1000 holding two pointers and two strings.
    fn image() -> Vec<u8> {
        let base = 0x1_4000_0000u64;
        let mut b = vec![0u8; 0x400];
        b[..2].copy_from_slice(b"MZ");
        b[0x3C..0x40].copy_from_slice(&0x40u32.to_le_bytes());
        b[0x40..0x44].copy_from_slice(b"PE\0\0");
        let coff = 0x44;
        b[coff + 2..coff + 4].copy_from_slice(&1u16.to_le_bytes());
        b[coff + 16..coff + 18].copy_from_slice(&0xF0u16.to_le_bytes());
        let opt = coff + 20;
        b[opt..opt + 2].copy_from_slice(&0x20Bu16.to_le_bytes());
        b[opt + 24..opt + 32].copy_from_slice(&base.to_le_bytes());
        let s = opt + 0xF0;
        b[s + 8..s + 12].copy_from_slice(&0x200u32.to_le_bytes());
        b[s + 12..s + 16].copy_from_slice(&0x1000u32.to_le_bytes());
        b[s + 16..s + 20].copy_from_slice(&0x200u32.to_le_bytes());
        b[s + 20..s + 24].copy_from_slice(&0x200u32.to_le_bytes());
        let data = 0x200;
        b[data..data + 8].copy_from_slice(&(base + 0x1010).to_le_bytes());
        b[data + 8..data + 16].copy_from_slice(&(base + 0x1020).to_le_bytes());
        b[data + 0x10..data + 0x15].copy_from_slice(b"alpha");
        b[data + 0x20..data + 0x24].copy_from_slice(b"beta");
        b
    }

    #[test]
    fn reads_a_string_table_by_address() {
        let b = image();
        let img = Image::parse(&b).unwrap();
        assert_eq!(img.string_table(0x1_4000_1000, 2).unwrap(), ["alpha", "beta"]);
        assert!(img.u64_at(0x1_4000_5000).is_err());
        assert!(Image::parse(b"MZ").is_err());
    }
}
