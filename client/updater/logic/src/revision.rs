//! The patch revision a Skhid game exe says in its sign-in, read from the
//! exe file: what the login server compares with `SRP_MIN_PATCH_REVISION`
//! (`srp-026g/client-patches/patch_026g.py`, `REVISION_SITE`;
//! `docs/notes/client_patch.md`).
//!
//! The patch turns `mov cl,[9DAE9Eh]` at 0x380862 of `sign_in_on_connected`
//! (`8A 0D 9E AE 9D 00`) into `mov byte [esp+27h],<revision>` / `nop`
//! (`C6 44 24 27 <revision> 90`). The exe's sections say where that address
//! is in the file.

/// Where the revision byte's instruction is (a virtual address of the
/// game's image, whose base is 0x10000).
pub const SITE_VA: u32 = 0x0038_0862;
/// The instruction's length: what is read at its place.
pub const SITE_LEN: usize = 6;
/// The original exe's bytes there: a game without the patch says 0.
const ORIGINAL: [u8; SITE_LEN] = [0x8A, 0x0D, 0x9E, 0xAE, 0x9D, 0x00];
/// How much of the file's start [`site_offset`] needs at most: the DOS and
/// PE headers with the section table.
pub const HEADERS_LEN: usize = 4096;

/// What the exe says.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Revision {
    /// A patched exe: its revision.
    Patched(u8),
    /// The original's instruction: revision 0.
    Stock,
    /// Other bytes: not this game's exe, or one whose site moved. Nothing is
    /// known of its revision.
    Unknown,
}

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(bytes.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

/// Where [`SITE_VA`] is in the file, from its first bytes (`headers`, up to
/// [`HEADERS_LEN`]): `None` when they are not a 32-bit PE's or no section
/// holds the whole instruction in the file.
pub fn site_offset(headers: &[u8]) -> Option<u64> {
    if headers.get(..2)? != b"MZ" {
        return None;
    }
    let pe = u32_at(headers, 0x3C)? as usize;
    if headers.get(pe..pe + 4)? != b"PE\0\0" {
        return None;
    }
    let sections = usize::from(u16_at(headers, pe + 6)?);
    let optional_size = usize::from(u16_at(headers, pe + 20)?);
    let optional = pe + 24;
    // PE32 (0x10B): the image base is a 32-bit number at +28.
    if u16_at(headers, optional)? != 0x10B {
        return None;
    }
    let base = u32_at(headers, optional + 28)?;
    let rva = SITE_VA.checked_sub(base)?;
    let table = optional + optional_size;
    (0..sections).find_map(|index| {
        let entry = table + index * 40;
        let virtual_size = u32_at(headers, entry + 8)?;
        let virtual_address = u32_at(headers, entry + 12)?;
        let raw_size = u32_at(headers, entry + 16)?;
        let raw = u32_at(headers, entry + 20)?;
        let within = rva.checked_sub(virtual_address)?;
        let end = within.checked_add(SITE_LEN as u32)?;
        (end <= virtual_size && end <= raw_size).then(|| u64::from(raw) + u64::from(within))
    })
}

/// What the instruction at the site says.
pub fn revision_of(site: &[u8]) -> Revision {
    match site {
        [0xC6, 0x44, 0x24, 0x27, revision, 0x90] => Revision::Patched(*revision),
        _ if site == ORIGINAL => Revision::Stock,
        _ => Revision::Unknown,
    }
}

impl Revision {
    /// The number the game says in its sign-in, when it is known.
    pub fn number(self) -> Option<u8> {
        match self {
            Revision::Patched(revision) => Some(revision),
            Revision::Stock => Some(0),
            Revision::Unknown => None,
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;

    /// A small PE32 whose one section holds the site at file offset
    /// `0x600 + 0x862`, as the game's `.text` holds it (base 0x10000).
    pub fn exe(site: &[u8]) -> Vec<u8> {
        let mut file = vec![0_u8; 0x1000];
        file[..2].copy_from_slice(b"MZ");
        file[0x3C..0x40].copy_from_slice(&0x80_u32.to_le_bytes());
        file[0x80..0x84].copy_from_slice(b"PE\0\0");
        file[0x86..0x88].copy_from_slice(&1_u16.to_le_bytes());
        file[0x94..0x96].copy_from_slice(&0xE0_u16.to_le_bytes());
        file[0x98..0x9A].copy_from_slice(&0x10B_u16.to_le_bytes());
        file[0x98 + 28..0x98 + 32].copy_from_slice(&0x10000_u32.to_le_bytes());
        let section = 0x98 + 0xE0;
        file[section..section + 5].copy_from_slice(b".text");
        for (at, value) in [(8, 0x1000_u32), (12, 0x37_0000), (16, 0x1000), (20, 0x600)] {
            file[section + at..section + at + 4].copy_from_slice(&value.to_le_bytes());
        }
        file.resize(0x1600, 0);
        file[0xE62..0xE62 + site.len()].copy_from_slice(site);
        file
    }

    #[test]
    fn the_site_is_found_by_the_sections() {
        let file = exe(&[0xC6, 0x44, 0x24, 0x27, 27, 0x90]);
        let at = site_offset(&file[..HEADERS_LEN]).unwrap() as usize;
        assert_eq!(at, 0xE62);
        assert_eq!(revision_of(&file[at..at + SITE_LEN]), Revision::Patched(27));
        assert_eq!(Revision::Patched(27).number(), Some(27));
    }

    #[test]
    fn the_original_says_zero_and_other_bytes_nothing() {
        assert_eq!(revision_of(&ORIGINAL), Revision::Stock);
        assert_eq!(Revision::Stock.number(), Some(0));
        assert_eq!(revision_of(&[0x90; 6]), Revision::Unknown);
        assert_eq!(
            revision_of(&[0xC6, 0x44, 0x24, 0x28, 27, 0x90]),
            Revision::Unknown
        );
        assert_eq!(revision_of(&[0xC6, 0x44]), Revision::Unknown);
        assert_eq!(Revision::Unknown.number(), None);
    }

    #[test]
    fn what_is_not_such_an_exe_has_no_site() {
        let file = exe(&ORIGINAL);
        assert_eq!(site_offset(b""), None);
        assert_eq!(site_offset(b"MZ"), None);
        let mut spoilt = file.clone();
        spoilt[0x80] = b'X';
        assert_eq!(site_offset(&spoilt), None);
        // A PE32+ (64-bit) is not the game's.
        let mut wide = file.clone();
        wide[0x98..0x9A].copy_from_slice(&0x20B_u16.to_le_bytes());
        assert_eq!(site_offset(&wide), None);
        // Another base: the address is in no section.
        let mut moved = file.clone();
        moved[0x98 + 28..0x98 + 32].copy_from_slice(&0x400000_u32.to_le_bytes());
        assert_eq!(site_offset(&moved), None);
        // A section whose bytes in the file end before the instruction does.
        let mut short = file.clone();
        let raw_size = 0x98 + 0xE0 + 16;
        short[raw_size..raw_size + 4].copy_from_slice(&0x864_u32.to_le_bytes());
        assert_eq!(site_offset(&short), None);
        // Headers cut short.
        assert_eq!(site_offset(&file[..0x100]), None);
    }

    /// `SKHID_TEST_GAME_EXE=<a released survarium_skhid.exe>:<its revision>`:
    /// the real exe read as the updater reads it.
    #[test]
    fn a_real_game_exe_says_its_revision() {
        let Ok(given) = std::env::var("SKHID_TEST_GAME_EXE") else {
            return;
        };
        let (path, revision) = given.rsplit_once(':').expect("<path>:<revision>");
        let file = std::fs::read(path).unwrap();
        let at = site_offset(&file[..HEADERS_LEN]).unwrap() as usize;
        assert_eq!(
            revision_of(&file[at..at + SITE_LEN]),
            Revision::Patched(revision.parse().unwrap())
        );
    }
}
